//! Установка и запуск `.flatpak`-бандлов.
//!
//! OpenRun не распаковывает Flatpak сам: установкой занимается системный
//! `flatpak`. Мы только запоминаем идентификаторы приложений, которые
//! установили через OpenRun, и делаем для них ярлыки.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};

use super::i18n::t;

pub struct Installed {
    pub id: String,
    pub name: String,
}

pub struct App {
    pub id: String,
    pub name: String,
}

/// Внутри Flatpak данные приложения лежат в ~/.var/app/..., а маркеры
/// установленных бандлов лучше хранить в обычном домашнем каталоге:
/// тогда они переживут переустановку OpenRun.
fn data_dir() -> PathBuf {
    if super::in_flatpak() {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".local")
            .join("share")
    } else {
        dirs::data_dir().unwrap_or_else(|| PathBuf::from("."))
    }
}

fn marker_dir() -> PathBuf {
    data_dir().join(crate::core::APP_DIR).join("flatpak")
}

/// Внутри песочницы Flatpak системного `flatpak` нет, поэтому команды
/// выполняются на хосте через `flatpak-spawn`.
fn flatpak_cmd() -> Command {
    if super::in_flatpak() {
        let mut cmd = Command::new("flatpak-spawn");
        cmd.arg("--host").arg("flatpak");
        cmd
    } else {
        Command::new("flatpak")
    }
}

pub fn available() -> bool {
    flatpak_cmd()
        .arg("--version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

fn output_error(out: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let text = if stderr.trim().is_empty() {
        stdout
    } else {
        stderr
    };
    text.trim().to_string()
}

/// Ищет `app/org.example.App/x86_64/stable` в выводе flatpak.
fn parse_app_id(text: &str) -> Option<String> {
    for raw in text.split_whitespace() {
        let token = raw.trim_matches(|c: char| {
            matches!(
                c,
                ',' | ';' | ':' | '(' | ')' | '[' | ']' | '\'' | '"' | '.'
            )
        });
        if let Some(rest) = token.strip_prefix("app/") {
            let id = rest.split('/').next().unwrap_or_default();
            if id.contains('.') {
                return Some(id.to_string());
            }
        }
    }
    None
}

fn installed_ids() -> Vec<String> {
    let Ok(out) = flatpak_cmd()
        .args(["list", "--app", "--columns=application"])
        .output()
    else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

fn is_installed(id: &str) -> bool {
    flatpak_cmd()
        .args(["info", id])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn app_name(id: &str) -> Option<String> {
    let out = flatpak_cmd()
        .args(["list", "--app", "--columns=application,name"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|line| {
            let (app_id, name) = line.split_once('\t')?;
            (app_id.trim() == id && !name.trim().is_empty()).then(|| name.trim().to_string())
        })
}

fn marker_path(id: &str) -> PathBuf {
    marker_dir().join(format!("{id}.app"))
}

fn save_marker(id: &str, name: &str) -> Result<()> {
    if id.is_empty() || id.contains('/') {
        bail!(
            "{}",
            t("некорректный идентификатор Flatpak", "invalid Flatpak ID")
        );
    }
    fs::create_dir_all(marker_dir())?;
    fs::write(marker_path(id), format!("{name}\n"))?;
    Ok(())
}

fn remove_marker(id: &str) {
    let _ = fs::remove_file(marker_path(id));
}

/// Устанавливает `.flatpak`-бандл в пользовательскую установку Flatpak.
pub fn install_file(path: &Path) -> Result<Installed> {
    if !available() {
        bail!(
            "{}",
            t(
                "не найден flatpak. Установите его: sudo apt install flatpak",
                "flatpak was not found. Install it with: sudo apt install flatpak"
            )
        );
    }

    let path = fs::canonicalize(path).context(t("файл не найден", "file not found"))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("package.flatpak");
    if !file_name.to_lowercase().ends_with(".flatpak") {
        bail!(
            "{file_name}: {}",
            t("это не .flatpak", "not a .flatpak file")
        );
    }

    let before = installed_ids();
    let out = flatpak_cmd()
        .args(["install", "--user", "--noninteractive", "-y"])
        .arg(&path)
        .output()
        .context(t("не удалось запустить flatpak", "failed to run flatpak"))?;

    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    if !out.status.success() {
        bail!("{}", output_error(&out));
    }

    let after = installed_ids();
    let id = parse_app_id(&text)
        .or_else(|| {
            after
                .iter()
                .find(|id| !before.iter().any(|old| old == *id))
                .cloned()
        })
        .context(t(
            "не удалось определить идентификатор установленного Flatpak",
            "could not determine the installed Flatpak ID",
        ))?;

    let name = app_name(&id).unwrap_or_else(|| id.clone());
    save_marker(&id, &name)?;
    Ok(Installed { id, name })
}

pub fn list_installed() -> Vec<App> {
    let mut apps = Vec::new();
    let Ok(entries) = fs::read_dir(marker_dir()) else {
        return apps;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("app") {
            continue;
        }
        let Some(id) = path.file_stem().and_then(|name| name.to_str()) else {
            continue;
        };
        if !is_installed(id) {
            let _ = fs::remove_file(&path);
            continue;
        }
        let name = fs::read_to_string(&path)
            .ok()
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty())
            .unwrap_or_else(|| id.to_string());
        apps.push(App {
            id: id.to_string(),
            name,
        });
    }
    apps.sort_by_key(|app| app.name.to_lowercase());
    apps
}

pub fn launch(id: &str) -> Result<()> {
    let mut cmd = flatpak_cmd();
    cmd.args(["run", id]).stdin(Stdio::null());
    let mut child = cmd.spawn().context(t(
        "не удалось запустить Flatpak-приложение",
        "failed to launch the Flatpak app",
    ))?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

pub fn uninstall(id: &str) -> Result<()> {
    let out = flatpak_cmd()
        .args(["uninstall", "--user", "--noninteractive", "-y", id])
        .output()
        .context(t("не удалось запустить flatpak", "failed to run flatpak"))?;
    if !out.status.success() {
        bail!("{}", output_error(&out));
    }
    remove_marker(id);
    super::shortcut::remove_flatpak_shortcut(id);
    Ok(())
}

/// Пересоздаёт ярлыки для всех Flatpak-приложений, установленных через OpenRun.
pub fn refresh_launchers() -> usize {
    let mut count = 0;
    for app in list_installed() {
        if super::shortcut::create_flatpak_shortcut(&app.id, &app.name).is_ok() {
            count += 1;
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::parse_app_id;

    #[test]
    fn parses_installed_app_id() {
        assert_eq!(
            parse_app_id("Installing app/org.example.App/x86_64/stable\n"),
            Some("org.example.App".to_string())
        );
    }

    #[test]
    fn ignores_runtime_lines() {
        assert_eq!(parse_app_id("runtime/org.gnome.Platform/x86_64/49\n"), None);
    }
}
