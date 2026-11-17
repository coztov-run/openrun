//! Встроенный рантайм OpenRun.
//!
//! Приложение не требует устанавливать что-либо в систему: при первом запуске
//! оно скачивает самодостаточную сборку в свой каталог данных и дальше работает
//! только с ней. Пока сборка не готова, используется системный рантайм.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use anyhow::{bail, Context, Result};

use super::i18n::t;

/// API последнего релиза Wine-Builds (самодостаточные сборки без зависимостей от системы).
const RELEASES_API: &str = "https://api.github.com/repos/Kron4ek/Wine-Builds/releases/latest";
/// Запасная ссылка, если GitHub API недоступен или исчерпан лимит запросов.
const FALLBACK_URL: &str = "https://github.com/Kron4ek/Wine-Builds/releases/download/11.22/wine-11.22-amd64-wow64.tar.xz";

/// Сообщения о ходе установки встроенного OpenRun.
pub enum Progress {
    Status(String),
    Download { done: u64, total: u64 },
    Extract,
}

fn data_root() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(crate::core::APP_DIR)
}

pub fn root() -> PathBuf {
    data_root().join("runtime")
}

pub fn wine_dir() -> PathBuf {
    root().join("wine")
}

/// Путь к исполняемому файлу рантайма: сначала встроенный, затем системный.
pub fn wine_bin() -> PathBuf {
    let bundled = wine_dir().join("bin/wine");
    if bundled.exists() {
        bundled
    } else {
        PathBuf::from("wine")
    }
}

pub fn is_bundled_ready() -> bool {
    wine_dir().join("bin/wine").exists()
}

/// Скачивает и распаковывает портативный рантайм OpenRun в каталог данных приложения.
pub fn install(progress: &mut dyn FnMut(Progress)) -> Result<()> {
    let root = root();
    fs::create_dir_all(&root)?;

    progress(Progress::Status(t(
        "Ищу последнюю сборку OpenRun…",
        "Looking for the latest OpenRun build…",
    ).to_string()));
    let url = latest_asset_url().unwrap_or_else(|_| FALLBACK_URL.to_string());

    let archive = root.join("wine-runtime.tar.xz");
    progress(Progress::Status(t(
        "Скачиваю OpenRun (около 100 МБ)…",
        "Downloading OpenRun (about 100 MB)…",
    ).to_string()));
    download(&url, &archive, progress)?;

    progress(Progress::Extract);
    extract_archive(&archive, &root)?;
    let _ = fs::remove_file(&archive);

    if !is_bundled_ready() {
        bail!("{}", t("после распаковки не найден исполняемый файл OpenRun", "OpenRun executable not found after extraction"));
    }
    progress(Progress::Status(t("OpenRun готов", "OpenRun is ready").to_string()));
    Ok(())
}

fn latest_asset_url() -> Result<String> {
    let body = ureq::get(RELEASES_API)
        .set("User-Agent", "openrun")
        .timeout(Duration::from_secs(30))
        .call()
        .context("GitHub API недоступен")?
        .into_string()
        .context("не удалось прочитать ответ GitHub")?;

    let marker = "\"browser_download_url\"";
    for (idx, _) in body.match_indices(marker) {
        let rest = &body[idx + marker.len()..];
        // Дальше идёт `: "https://…"` — берём первую строку в кавычках.
        let Some(first) = rest.find('"') else { continue };
        let rest = &rest[first + 1..];
        let Some(second) = rest.find('"') else { continue };
        let url = &rest[..second];
        if url.ends_with("amd64-wow64.tar.xz") {
            return Ok(url.to_string());
        }
    }
    bail!("в релизе GitHub не найдена сборка amd64-wow64")
}

fn download(url: &str, dest: &Path, progress: &mut dyn FnMut(Progress)) -> Result<()> {
    let resp = ureq::get(url)
        .set("User-Agent", "openrun")
        .timeout(Duration::from_secs(1800))
        .call()
        .context(t("не удалось скачать OpenRun с GitHub", "failed to download OpenRun from GitHub"))?;

    let total = resp
        .header("content-length")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);

    let mut reader = resp.into_reader();
    let mut file = fs::File::create(dest).context("не удалось создать файл архива")?;
    let mut buf = [0u8; 64 * 1024];
    let mut done = 0u64;
    let mut last_step = 0u64;
    let step = if total > 0 { (total / 100).max(1) } else { 0 };

    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        done += n as u64;
        if step == 0 || done / step != last_step / step {
            last_step = done;
            progress(Progress::Download { done, total });
        }
    }
    progress(Progress::Download { done, total });
    file.flush().ok();
    Ok(())
}

fn extract_archive(archive: &Path, root: &Path) -> Result<()> {
    let out = Command::new("tar")
        .arg("-xJf")
        .arg(archive)
        .arg("-C")
        .arg(root)
        .output()
        .context(t("не удалось запустить tar (он нужен для распаковки OpenRun)", "failed to run tar (it is required to extract OpenRun)"))?;
    if !out.status.success() {
        bail!(
            "{}: {}",
            t("tar не смог распаковать архив OpenRun", "tar failed to extract the OpenRun archive"),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }

    let mut extracted = None;
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name != "wine" && name.starts_with("wine") {
            extracted = Some(entry.path());
            break;
        }
    }

    let extracted = extracted.context(t("в архиве не найден каталог OpenRun", "OpenRun directory not found in the archive"))?;
    let target = wine_dir();
    if target.exists() {
        fs::remove_dir_all(&target)?;
    }
    fs::rename(&extracted, &target).context(t("не удалось перенести OpenRun в каталог приложения", "failed to move OpenRun into the app data directory"))?;
    Ok(())
}
