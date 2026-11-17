//! Установка .deb-пакетов на не-Debian системы (Arch, CachyOS и т.д.).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

use super::i18n::t;

pub struct Installed {
    pub name: String,
    pub launchers: usize,
    pub warnings: Vec<String>,
}

fn data_dir() -> PathBuf {
    dirs::data_dir().unwrap_or_else(|| PathBuf::from("."))
}

pub fn apps_root() -> PathBuf {
    data_dir().join(crate::core::APP_DIR).join("deb")
}

pub fn install_dir(name: &str) -> PathBuf {
    apps_root().join(name)
}

fn launchers_dir() -> PathBuf {
    data_dir().join("applications")
}

pub fn tool_available(tool: &str) -> bool {
    Command::new(tool).arg("--version").output().is_ok()
}

fn check_tools() -> Result<()> {
    for tool in ["tar", "xz"] {
        if !tool_available(tool) {
            bail!("{}: {tool}", t("не найдена утилита, без неё нельзя распаковать .deb", "required tool not found, cannot extract the .deb"));
        }
    }
    Ok(())
}

pub fn install_file(deb_path: &Path) -> Result<Installed> {
    check_tools()?;

    let deb_path = fs::canonicalize(deb_path).context(t("файл не найден", "file not found"))?;
    let file_name = deb_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("package.deb")
        .to_string();
    if !file_name.to_lowercase().ends_with(".deb") {
        bail!("{file_name}: {}", t("это не .deb", "not a .deb file"));
    }

    let name = package_name(&file_name);
    let dest = apps_root().join(&name);
    if dest.exists() {
        fs::remove_dir_all(&dest)?;
    }
    fs::create_dir_all(&dest)?;
    extract(&deb_path, &dest)?;

    let launchers = export_desktop(&dest, &name)?;
    copy_icons(&dest)?;

    let mut warnings = vec![
        t(
            "Зависимости из поля Depends не ставятся автоматически. Если программа не запускается, установите их через pacman или yay.",
            "Dependencies from the Depends field are not installed automatically. If the program does not start, install them via pacman or yay.",
        ).to_string(),
    ];
    if !dest.join("usr").exists() {
        warnings.push(t(
            "В пакете нет каталога usr/. Возможно, это не обычное приложение.",
            "The package has no usr/ directory. It may not be a regular application.",
        ).to_string());
    }

    Ok(Installed {
        name,
        launchers,
        warnings,
    })
}

fn run(cmd: &mut Command) -> Result<()> {
    let out = cmd.output().context(t("не удалось запустить внешнюю утилиту", "failed to run an external tool"))?;
    if !out.status.success() {
        bail!("{}", String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(())
}

fn extract(deb: &Path, dest: &Path) -> Result<()> {
    // .deb — это ar-архив; разбираем его сами, чтобы не требовать binutils (ar).
    let members = extract_ar(deb, dest)?;

    let data = members
        .into_iter()
        .find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("data.tar"))
        })
        .context(t("в пакете нет data.tar", "the package has no data.tar"))?;

    run(Command::new("tar").arg("-xf").arg(&data).arg("-C").arg(dest))?;

    for entry in fs::read_dir(dest)? {
        let path = entry?.path();
        let n = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        if n == "debian-binary" || n.starts_with("control.tar") || n.starts_with("data.tar") {
            let _ = fs::remove_file(&path);
        }
    }
    Ok(())
}

/// Разбирает формат ar и сохраняет элементы во временный каталог.
fn extract_ar(archive: &Path, dest: &Path) -> Result<Vec<PathBuf>> {
    let data = fs::read(archive).context(t("не удалось прочитать .deb", "failed to read the .deb"))?;
    if data.len() < 8 || &data[..8] != b"!<arch>\n" {
        bail!("{}", t("файл не похож на пакет .deb (нет заголовка ar)", "the file does not look like a .deb package (no ar header)"));
    }

    let mut pos = 8usize;
    let mut files = Vec::new();
    while pos + 60 <= data.len() {
        let header = &data[pos..pos + 60];
        let name = String::from_utf8_lossy(&header[..16])
            .trim()
            .trim_end_matches('/')
            .to_string();
        let size: usize = String::from_utf8_lossy(&header[48..58])
            .trim()
            .parse()
            .context(t("повреждён ar-архив", "corrupted ar archive"))?;

        let start = pos + 60;
        let end = start
            .checked_add(size)
            .filter(|e| *e <= data.len())
            .context(t("повреждён ar-архив", "corrupted ar archive"))?;

        // Служебные элементы (таблица символов, длинные имена) пропускаем.
        if !name.is_empty() && name != "/" && !name.starts_with("//") {
            let out = dest.join(&name);
            if let Some(parent) = out.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&out, &data[start..end])?;
            files.push(out);
        }

        pos = end + (size % 2);
    }
    Ok(files)
}

fn export_desktop(dest: &Path, name: &str) -> Result<usize> {
    let apps = dest.join("usr/share/applications");
    if !apps.is_dir() {
        return Ok(0);
    }
    let target_dir = launchers_dir();
    fs::create_dir_all(&target_dir)?;
    remove_legacy_launchers(name);

    let mut count = 0;
    for entry in fs::read_dir(&apps)? {
        let path = entry?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
            continue;
        }
        let text = fs::read_to_string(&path)?;
        let fixed = text
            .lines()
            .map(|line| {
                for key in ["Exec=", "TryExec="] {
                    if let Some(value) = line.strip_prefix(key) {
                        return format!("{key}{}", fix_exec(value, dest));
                    }
                }
                line.to_string()
            })
            .collect::<Vec<_>>()
            .join("\n");

        let file = path.file_name().unwrap_or_default().to_string_lossy();
        fs::write(target_dir.join(format!("openrun-{name}-{file}")), fixed + "\n")?;
        count += 1;
    }
    Ok(count)
}

/// Переписывает путь к программе в Exec/TryExec на распакованный пакет.
fn fix_exec(value: &str, dest: &Path) -> String {
    if value.contains("/usr/") {
        return value.replacen("/usr/", &format!("{}/usr/", dest.display()), 1);
    }

    let trimmed = value.trim_start();
    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let program = parts.next().unwrap_or_default();
    let rest = parts.next().unwrap_or_default();

    if !program.is_empty() && !program.contains('/') {
        let candidate = dest.join("usr/bin").join(program);
        if candidate.exists() {
            let mut out = candidate.display().to_string();
            if !rest.is_empty() {
                out.push(' ');
                out.push_str(rest);
            }
            return out;
        }
    }
    value.to_string()
}

fn copy_icons(dest: &Path) -> Result<()> {
    let src = dest.join("usr/share/icons");
    if src.is_dir() {
        copy_dir(&src, &data_dir().join("icons"))?;
    }
    Ok(())
}

fn copy_dir(src: &Path, dst: &Path) -> Result<usize> {
    fs::create_dir_all(dst)?;
    let mut n = 0;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let to = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            n += copy_dir(&entry.path(), &to)?;
        } else {
            fs::copy(entry.path(), &to)?;
            n += 1;
        }
    }
    Ok(n)
}

fn launcher_files(name: &str) -> Vec<PathBuf> {
    let prefixes = [format!("openrun-{name}-"), format!("wm-{name}-")];
    let mut files: Vec<PathBuf> = fs::read_dir(launchers_dir())
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                        prefixes.iter().any(|prefix| n.starts_with(prefix))
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    files
}

/// Удаляет ярлыки прежнего имени `wm-*`, оставшиеся от старой версии.
fn remove_legacy_launchers(name: &str) {
    let prefix = format!("wm-{name}-");
    if let Ok(rd) = fs::read_dir(launchers_dir()) {
        for entry in rd.filter_map(|e| e.ok()) {
            if entry.file_name().to_string_lossy().starts_with(&prefix) {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
}

pub fn has_launcher(name: &str) -> bool {
    !launcher_files(name).is_empty()
}

/// Запускает программу из установленного .deb через её .desktop-ярлык.
pub fn launch(name: &str) -> Result<()> {
    let files = launcher_files(name);
    let file = files.first().context(t("у пакета нет ярлыка для запуска", "the package has no launcher"))?;
    let text = fs::read_to_string(file)?;
    let exec = text
        .lines()
        .find_map(|line| line.strip_prefix("Exec="))
        .context(t("в ярлыке нет строки Exec", "the launcher has no Exec line"))?;

    let mut parts = split_exec(exec);
    parts.retain(|p| !p.starts_with('%'));
    let program = parts.first().context(t("пустая команда запуска", "empty launch command"))?.clone();

    // Если в Exec указано только имя программы, ищем её в распакованном пакете.
    let program_path = if program.contains('/') {
        PathBuf::from(&program)
    } else {
        let candidate = install_dir(name).join("usr/bin").join(&program);
        if candidate.exists() {
            candidate
        } else {
            PathBuf::from(&program)
        }
    };

    let mut cmd = Command::new(&program_path);
    cmd.args(&parts[1..]);
    cmd.stdin(std::process::Stdio::null());
    let mut child = cmd.spawn().context(t("не удалось запустить программу", "failed to launch the program"))?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

/// Разбирает строку Exec с учётом кавычек.
fn split_exec(input: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut chars = input.chars();

    while let Some(c) = chars.next() {
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                } else if c == '\\' {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                } else {
                    current.push(c);
                }
            }
            None => {
                if c == '"' || c == '\'' {
                    quote = Some(c);
                } else if c == '\\' {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                } else if c.is_whitespace() {
                    if !current.is_empty() {
                        parts.push(current.clone());
                        current.clear();
                    }
                } else {
                    current.push(c);
                }
            }
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts
}

/// Пересоздаёт ярлыки и копирует иконки для всех распакованных .deb-пакетов.
/// Нужен, если ярлыки были удалены или кеш значков устарел.
pub fn refresh_launchers() -> usize {
    let mut count = 0;
    for name in list_installed() {
        let dest = install_dir(&name);
        if let Ok(n) = export_desktop(&dest, &name) {
            count += n;
        }
        let _ = copy_icons(&dest);
    }
    count
}

pub fn list_installed() -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(apps_root())
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| !n.starts_with('.'))
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

pub fn uninstall(name: &str) -> Result<()> {
    let dir = install_dir(name);
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }
    remove_legacy_launchers(name);
    let prefix = format!("openrun-{name}-");
    if let Ok(rd) = fs::read_dir(launchers_dir()) {
        for entry in rd.filter_map(|e| e.ok()) {
            if entry.file_name().to_string_lossy().starts_with(&prefix) {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
    Ok(())
}

pub fn package_name(file: &str) -> String {
    let base = file.trim_end_matches(".deb").split('_').next().unwrap_or("package");
    let clean: String = base
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '+'))
        .collect();
    if clean.is_empty() {
        "package".to_string()
    } else {
        clean
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_name_from_file() {
        assert_eq!(package_name("foo_1.2_amd64.deb"), "foo");
        assert_eq!(package_name("code_1.90.0-1_amd64.deb"), "code");
    }
}
