//! Ярлыки (.desktop): для самого OpenRun и для запущенных Windows-программ.

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};

use super::config::AppEntry;
use super::deb;
use super::i18n::t;
use super::prefix::slug;
use super::runtime;

const SELF_ICON: &[u8] =
    include_bytes!("../../data/icons/hicolor/scalable/apps/io.github.coztov_run.OpenRun.svg");

/// MIME-типы, которые OpenRun умеет открывать двойным кликом.
///
/// Разные дистрибутивы определяют `.exe` по-разному, поэтому перечислены
/// все распространённые варианты. `.deb` и `.msi` — стандартные типы.
const MIME_TYPES: &[&str] = &[
    "application/x-msdownload",
    "application/x-ms-dos-executable",
    "application/vnd.microsoft.portable-executable",
    "application/x-msi",
    "application/vnd.debian.binary-package",
];

const MIME_LIST: &str = "application/x-msdownload;application/x-ms-dos-executable;application/vnd.microsoft.portable-executable;application/x-msi;application/vnd.debian.binary-package;";

fn data_dir() -> PathBuf {
    dirs::data_dir().unwrap_or_else(|| PathBuf::from("."))
}

/// Внутри Flatpak личные каталоги пользователя подменены песочницей.
/// Ярлыки и MIME-ассоциации должны попадать в настоящий домашний каталог,
/// поэтому для них используется отдельный путь.
fn in_flatpak() -> bool {
    if std::env::var("FLATPAK_ID").is_ok_and(|id| id == crate::APP_ID) {
        return true;
    }
    // /.flatpak-info есть и у чужих Flatpak-приложений (например, у терминала),
    // поэтому проверяем именно идентификатор OpenRun.
    std::fs::read_to_string("/.flatpak-info")
        .map(|text| text.contains(&format!("name={}", crate::APP_ID)))
        .unwrap_or(false)
}

fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

fn host_data_dir() -> PathBuf {
    if in_flatpak() {
        home_dir().join(".local").join("share")
    } else {
        data_dir()
    }
}

fn apps_dir() -> PathBuf {
    host_data_dir().join("applications")
}

fn host_config_dir() -> PathBuf {
    if in_flatpak() {
        home_dir().join(".config")
    } else {
        dirs::config_dir().unwrap_or_else(|| PathBuf::from("."))
    }
}

/// Каталоги рабочего стола, если они существуют (русская и английская локали).
fn desktop_dirs() -> Vec<PathBuf> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    [home.join("Desktop"), home.join("Рабочий стол")]
        .into_iter()
        .filter(|p| p.is_dir())
        .collect()
}

/// Экранирует аргумент для строки Exec в .desktop-файле.
fn quote(arg: &str) -> String {
    let needs_quotes = arg
        .chars()
        .any(|c| c.is_whitespace() || matches!(c, '"' | '\'' | '\\' | '%'));
    if !needs_quotes {
        return arg.to_string();
    }
    let escaped = arg
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('%', "%%");
    format!("\"{escaped}\"")
}

fn write_icon() -> Result<PathBuf> {
    let dir = host_data_dir()
        .join("icons")
        .join("hicolor")
        .join("scalable")
        .join("apps");
    fs::create_dir_all(&dir)?;
    // Старые иконки прежних названий убираем, чтобы в меню не осталось двух значков.
    for old in ["org.example.WineManager.svg", "io.github.openrun.OpenRun.svg"] {
        let _ = fs::remove_file(dir.join(old));
    }

    // Каноническая иконка темы (её ищут по имени приложения).
    let canonical = dir.join(format!("{}.svg", crate::APP_ID));
    fs::write(&canonical, SELF_ICON)?;

    // Отдельное имя для ярлыков: рабочий стол и док перечитывают иконку,
    // даже если старая картинка уже попала в кеш под прежним именем.
    let path = dir.join("openrun.svg");
    fs::write(&path, SELF_ICON)?;
    Ok(path)
}

fn write_launcher(file_name: &str, content: &str) -> Result<PathBuf> {
    let dir = apps_dir();
    fs::create_dir_all(&dir)?;
    let path = dir.join(file_name);
    fs::write(&path, content)?;

    // Копия на рабочий стол (там нужен бит запуска).
    for desktop in desktop_dirs() {
        let copy = desktop.join(file_name);
        if fs::write(&copy, content).is_ok() {
            let _ = fs::set_permissions(&copy, fs::Permissions::from_mode(0o755));
        }
    }
    Ok(path)
}

fn remove_launcher(file_name: &str) {
    let _ = fs::remove_file(apps_dir().join(file_name));
    for desktop in desktop_dirs() {
        let _ = fs::remove_file(desktop.join(file_name));
    }
}

/// Создаёт ярлык для OpenRun (меню приложений и рабочий стол)
/// и назначает его обработчиком .exe/.msi/.deb.
pub fn create_self_shortcut() -> Result<PathBuf> {
    // В Flatpak путь /app/bin/openrun снаружи не существует: ярлык должен
    // запускать приложение через `flatpak run`.
    let exec = if in_flatpak() {
        format!("flatpak run {} %F", crate::APP_ID)
    } else {
        let exe = std::env::current_exe().context("не удалось определить путь к приложению")?;
        let exe = fs::canonicalize(&exe).unwrap_or(exe);
        format!("{} %F", quote(&exe.display().to_string()))
    };
    let icon = write_icon()?;
    // Ярлыки прежних названий и app-id больше не нужны.
    for old in ["wine-manager.desktop", "io.github.openrun.OpenRun.desktop"] {
        remove_launcher(old);
    }
    let content = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Version=1.0\n\
         Name={}\n\
         Comment={}\n\
         Exec={}\n\
         Icon={}\n\
         Terminal=false\n\
         Categories=Utility;System;\n\
         MimeType={}\n\
         StartupWMClass={}\n\
         StartupNotify=true\n",
        crate::core::APP_NAME,
        t("Менеджер Windows-программ", "Windows app manager"),
        exec,
        icon.display(),
        MIME_LIST,
        crate::APP_ID
    );
    let path = write_launcher("openrun.desktop", &content)?;
    register_mime_defaults();
    refresh_icon_cache();
    Ok(path)
}

/// Назначает OpenRun обработчиком по умолчанию в `~/.config/mimeapps.list`.
/// Существующие ассоциации сохраняются как запасной вариант.
fn register_mime_defaults() {
    let config = host_config_dir();
    let path = config.join("mimeapps.list");
    let old = fs::read_to_string(&path).unwrap_or_default();
    let new = update_mimeapps(&old);
    if new != old {
        let _ = fs::create_dir_all(&config);
        let _ = fs::write(path, new);
    }
}

fn update_mimeapps(text: &str) -> String {
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let existing = lines
        .iter()
        .position(|line| line.trim() == "[Default Applications]");
    let (start, end) = match existing {
        Some(start) => {
            let end = lines
                .iter()
                .enumerate()
                .skip(start + 1)
                .find(|(_, line)| {
                    let line = line.trim();
                    line.starts_with('[') && line.ends_with(']')
                })
                .map(|(idx, _)| idx)
                .unwrap_or(lines.len());
            (start, end)
        }
        None => {
            if !lines.is_empty() && !lines.last().is_some_and(|line| line.trim().is_empty()) {
                lines.push(String::new());
            }
            lines.push("[Default Applications]".to_string());
            (lines.len() - 1, lines.len())
        }
    };

    let mut entries: Vec<(String, String)> = Vec::new();
    let mut other: Vec<String> = Vec::new();
    for line in &lines[start + 1..end] {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            other.push(line.clone());
        } else if let Some((key, value)) = trimmed.split_once('=') {
            entries.push((key.trim().to_string(), value.trim().to_string()));
        } else {
            other.push(line.clone());
        }
    }

    for mime in MIME_TYPES {
        if let Some(pos) = entries
            .iter()
            .position(|(key, _)| key.as_str() == *mime)
        {
            let value = &mut entries[pos].1;
            let mut ids: Vec<String> = value
                .split(';')
                .map(str::trim)
                .filter(|id| !id.is_empty())
                .map(str::to_string)
                .collect();
            ids.retain(|id| id != "openrun.desktop");
            ids.insert(0, "openrun.desktop".to_string());
            *value = format!("{};", ids.join(";"));
        } else {
            entries.insert(0, ((*mime).to_string(), "openrun.desktop;".to_string()));
        }
    }

    let mut new_lines: Vec<String> = lines[..start + 1].to_vec();
    new_lines.extend(other);
    new_lines.extend(entries.into_iter().map(|(key, value)| format!("{key}={value}")));
    new_lines.extend_from_slice(&lines[end..]);

    let mut out = new_lines.join("\n");
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Разбирает `MimeType=` из .desktop-файла.
fn desktop_mime_types(text: &str) -> Vec<String> {
    text.lines()
        .find_map(|line| line.strip_prefix("MimeType="))
        .map(|value| {
            value
                .split(';')
                .map(str::trim)
                .filter(|mime| !mime.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Разбирает существующий `mimeinfo.cache`, сохраняя чужие записи.
fn parse_mime_cache(text: &str) -> BTreeMap<String, Vec<String>> {
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('[') || line.starts_with('#') {
            continue;
        }
        let Some((mime, handlers)) = line.split_once('=') else {
            continue;
        };
        let ids = map.entry(mime.trim().to_string()).or_default();
        for id in handlers.split(';').map(str::trim).filter(|id| !id.is_empty()) {
            if !ids.iter().any(|known| known == id) {
                ids.push(id.to_string());
            }
        }
    }
    map
}

/// Собирает новый `mimeinfo.cache`: чужие записи + найденные ярлыки,
/// а OpenRun ставится первым для поддерживаемых типов.
fn merge_mime_cache(old: &str, found: &BTreeMap<String, Vec<String>>) -> String {
    let mut map = parse_mime_cache(old);
    for (mime, ids) in found {
        let known = map.entry(mime.clone()).or_default();
        for id in ids {
            if !known.iter().any(|existing| existing == id) {
                known.push(id.clone());
            }
        }
    }
    for mime in MIME_TYPES {
        let known = map.entry((*mime).to_string()).or_default();
        known.retain(|id| id != "openrun.desktop");
        known.insert(0, "openrun.desktop".to_string());
    }

    let mut out = String::from("[MIME Cache]\n");
    for (mime, ids) in map {
        out.push_str(&format!("{mime}={};\n", ids.join(";")));
    }
    out
}

/// Обновляет `mimeinfo.cache` — часть файловых менеджеров ищет обработчики
/// только там. `update-desktop-database` есть не во всех дистрибутивах,
/// поэтому кеш при необходимости собирается вручную.
fn update_mime_cache() {
    let apps = apps_dir();
    if !apps.is_dir() {
        return;
    }

    let mut found: BTreeMap<String, Vec<String>> = BTreeMap::new();
    if let Ok(entries) = fs::read_dir(&apps) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("desktop") {
                continue;
            }
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            for mime in desktop_mime_types(&text) {
                found.entry(mime).or_default().push(name.to_string());
            }
        }
    }

    let path = apps.join("mimeinfo.cache");
    let old = fs::read_to_string(&path).unwrap_or_default();
    let new = merge_mime_cache(&old, &found);
    if new != old {
        let _ = fs::write(path, new);
    }
}

/// Обновляет кеши иконок и меню приложений, если нужные утилиты установлены.
fn refresh_icon_cache() {
    let hicolor = host_data_dir().join("icons").join("hicolor");
    if hicolor.is_dir() {
        let _ = Command::new("gtk-update-icon-cache")
            .arg("-f")
            .arg("-t")
            .arg(&hicolor)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }

    let apps = apps_dir();
    if apps.is_dir() {
        let _ = Command::new("update-desktop-database")
            .arg(&apps)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    update_mime_cache();
}

/// Пересоздаёт все ярлыки OpenRun, ярлыки Windows-программ и ярлыки .deb,
/// затем обновляет кеш значков. Используется кнопкой в настройках.
pub fn refresh_all(apps: &[AppEntry]) -> Result<()> {
    create_self_shortcut()?;
    for app in apps {
        let _ = create_app_shortcut(app);
    }
    let _ = deb::refresh_launchers();
    refresh_icon_cache();
    Ok(())
}

fn app_launcher_name(app: &AppEntry) -> String {
    format!("openrun-app-{}.desktop", slug(&app.name))
}

fn legacy_app_launcher_name(app: &AppEntry) -> String {
    format!("wm-app-{}.desktop", slug(&app.name))
}

/// Создаёт ярлык для Windows-программы, записанной в списке.
pub fn create_app_shortcut(app: &AppEntry) -> Result<PathBuf> {
    let is_msi = app
        .exe
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("msi"));
    let exec = if in_flatpak() {
        // Прямой путь к Wine внутри песочницы снаружи недоступен.
        // Ярлык просит OpenRun запустить запись по имени.
        format!(
            "flatpak run {} --launch-app {}",
            crate::APP_ID,
            quote(&app.name)
        )
    } else {
        let wine = runtime::wine_bin();
        let args = if is_msi {
            format!("msiexec /i {}", quote(&app.exe.display().to_string()))
        } else {
            quote(&app.exe.display().to_string())
        };
        format!(
            "env WINEPREFIX={} WINEDEBUG=-all WINEDLLOVERRIDES={} WINEFSYNC=1 WINEESYNC=1 {} {}",
            quote(&app.prefix.display().to_string()),
            quote("mscoree,mshtml=;winemenubuilder.exe=d"),
            quote(&wine.display().to_string()),
            args
        )
    };
    let content = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Version=1.0\n\
         Name={}\n\
         Comment={}\n\
         Exec={}\n\
         Icon=application-x-executable\n\
         Terminal=false\n\
         Categories=Utility;\n",
        app.name,
        t("Запуск через OpenRun", "Launch via OpenRun"),
        exec
    );
    // Ярлык старого имени `wm-app-*` заменяем новым `openrun-app-*`.
    remove_launcher(&legacy_app_launcher_name(app));
    write_launcher(&app_launcher_name(app), &content)
}

pub fn remove_app_shortcut(app: &AppEntry) {
    remove_launcher(&app_launcher_name(app));
    remove_launcher(&legacy_app_launcher_name(app));
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{desktop_mime_types, merge_mime_cache, update_mimeapps};

    #[test]
    fn adds_default_applications_section() {
        let out = update_mimeapps("");
        assert!(out.contains("[Default Applications]"));
        assert!(out.contains("application/x-msdownload=openrun.desktop;"));
        assert!(out.contains("application/x-msi=openrun.desktop;"));
        assert!(out.contains("application/vnd.debian.binary-package=openrun.desktop;"));
    }

    #[test]
    fn keeps_previous_handler_as_fallback() {
        let out = update_mimeapps(
            "[Default Applications]\napplication/x-msdownload=wine.desktop;\n",
        );
        assert!(out.contains("application/x-msdownload=openrun.desktop;wine.desktop;"));
    }

    #[test]
    fn preserves_other_sections_and_entries() {
        let out = update_mimeapps(
            "[Added Associations]\nfoo=bar;\n\n[Default Applications]\ntext/plain=gedit.desktop;\n",
        );
        assert!(out.contains("[Added Associations]\nfoo=bar;"));
        assert!(out.contains("text/plain=gedit.desktop;"));
        assert!(out.contains("application/x-msdownload=openrun.desktop;"));
    }

    #[test]
    fn reads_mime_types_from_desktop_entry() {
        let text = "[Desktop Entry]\nName=Test\nMimeType=application/x-msi;application/x-msdownload;\n";
        assert_eq!(
            desktop_mime_types(text),
            vec!["application/x-msi", "application/x-msdownload"]
        );
    }

    #[test]
    fn mime_cache_keeps_old_handlers_and_puts_openrun_first() {
        let old = "[MIME Cache]\napplication/x-msdownload=wine.desktop;\ntext/plain=gedit.desktop;\n";
        let found = BTreeMap::from([(
            "application/x-msdownload".to_string(),
            vec!["openrun.desktop".to_string()],
        )]);
        let out = merge_mime_cache(old, &found);
        assert!(out.contains("application/x-msdownload=openrun.desktop;wine.desktop;"));
        assert!(out.contains("text/plain=gedit.desktop;"));
        assert!(out.contains("application/x-msi=openrun.desktop;"));
    }
}
