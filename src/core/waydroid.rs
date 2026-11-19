//! Android-рантайм OpenRun: установка и запуск Android-приложений (.apk).
//!
//! Сейчас рантайм опирается на Waydroid: это системный компонент, который
//! нельзя запустить внутри песочницы Flatpak. OpenRun вызывает его на хосте
//! через `flatpak-spawn --host` и выступает менеджером APK.

use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{bail, Context, Result};

use super::i18n::t;

const APPIMAGE_RELEASES_API: &str =
    "https://api.github.com/repos/pkgforge-dev/Waydroid-AppImage/releases/latest";
const APPIMAGE_FALLBACK_URL: &str = "https://github.com/pkgforge-dev/Waydroid-AppImage/releases/download/1.6.3-1%402026-11-01_1793557813/Waydroid-1.6.3-1-anylinux-x86_64.AppImage";
const SYSTEM_OTA: &str = "https://ota.waydro.id/system/lineage/waydroid_x86_64/VANILLA.json";
const VENDOR_OTA: &str = "https://ota.waydro.id/vendor/waydroid_x86_64/MAINLINE.json";

/// Ход скачивания Android-рантайма.
pub enum Progress {
    Status(String),
    Download { file: String, done: u64, total: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct App {
    pub package: String,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Session {
    Running,
    Stopped,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Status {
    pub session: Session,
    pub container: Session,
}

impl Status {
    pub fn running(self) -> bool {
        self.session == Session::Running
    }
}

/// Каталог данных Android. Внутри Flatpak используем настоящий домашний
/// каталог, чтобы файлы были видны Android-рантайму на хосте.
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

fn staging_dir() -> PathBuf {
    data_dir().join(super::APP_DIR).join("android")
}

/// Каталог скачанного Android-рантайма.
pub fn runtime_root() -> PathBuf {
    data_dir().join(super::APP_DIR).join("android")
}

/// Скачан ли полный набор: AppImage, system и vendor.
pub fn runtime_downloaded() -> bool {
    let root = runtime_root();
    ["OpenRun-Android.AppImage", "system.zip", "vendor.zip"]
        .iter()
        .all(|name| {
            root.join(name)
                .metadata()
                .map(|meta| meta.is_file() && meta.len() > 0)
                .unwrap_or(false)
        })
}

fn json_string(body: &str, key: &str) -> Option<String> {
    let marker = format!("\"{key}\"");
    let index = body.find(marker.as_str())?;
    let rest = &body[index + marker.len()..];
    let colon = rest.find(':')?;
    let rest = &rest[colon + 1..];
    let first = rest.find('"')?;
    let rest = &rest[first + 1..];
    let second = rest.find('"')?;
    Some(rest[..second].to_string())
}

fn latest_appimage_url() -> Result<String> {
    let body = match ureq::get(APPIMAGE_RELEASES_API)
        .set("User-Agent", "openrun")
        .timeout(Duration::from_secs(30))
        .call()
    {
        Ok(response) => match response.into_string() {
            Ok(body) => body,
            Err(_) => return Ok(APPIMAGE_FALLBACK_URL.to_string()),
        },
        Err(_) => return Ok(APPIMAGE_FALLBACK_URL.to_string()),
    };

    let marker = "\"browser_download_url\"";
    for (index, _) in body.match_indices(marker) {
        let rest = &body[index + marker.len()..];
        let Some(first) = rest.find('"') else {
            continue;
        };
        let rest = &rest[first + 1..];
        let Some(second) = rest.find('"') else {
            continue;
        };
        let url = &rest[..second];
        if url.ends_with("x86_64.AppImage") {
            return Ok(url.to_string());
        }
    }
    Ok(APPIMAGE_FALLBACK_URL.to_string())
}

fn ota_url(channel: &str) -> Result<String> {
    let body = ureq::get(channel)
        .set("User-Agent", "openrun")
        .timeout(Duration::from_secs(30))
        .call()
        .context(t(
            "OTA-сервер Android недоступен",
            "Android OTA server is unavailable",
        ))?
        .into_string()
        .context(t(
            "не удалось прочитать ответ OTA",
            "failed to read the OTA response",
        ))?;
    json_string(&body, "url").context(t(
        "в ответе OTA нет ссылки на образ",
        "the OTA response has no image URL",
    ))
}

fn download(url: &str, dest: &Path, file: &str, progress: &mut dyn FnMut(Progress)) -> Result<()> {
    let response = ureq::get(url)
        .set("User-Agent", "openrun")
        .timeout(Duration::from_secs(1800))
        .call()
        .context(t(
            "не удалось скачать Android-рантайм",
            "failed to download the Android runtime",
        ))?;

    let total = response
        .header("content-length")
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);

    let file_name = dest
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("runtime.part");
    let part = dest.with_file_name(format!("{file_name}.part"));
    let mut reader = response.into_reader();
    let mut output = fs::File::create(&part).context(t(
        "не удалось создать файл Android-рантайма",
        "failed to create the Android runtime file",
    ))?;
    let mut buffer = [0u8; 64 * 1024];
    let mut done = 0u64;
    let mut last_step = 0u64;
    let step = if total > 0 {
        (total / 100).max(1)
    } else {
        1024 * 1024
    };

    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        output.write_all(&buffer[..count])?;
        done += count as u64;
        if done / step != last_step / step {
            last_step = done;
            progress(Progress::Download {
                file: file.to_string(),
                done,
                total,
            });
        }
    }
    output.flush().ok();
    drop(output);

    if total > 0 && done < total {
        let _ = fs::remove_file(&part);
        bail!(
            "{}: {done} / {total}",
            t("скачивание прервалось", "the download was interrupted")
        );
    }

    let extension = dest
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if let Err(error) = validate_download(&part, extension) {
        let _ = fs::remove_file(&part);
        return Err(error);
    }

    if let Err(error) = fs::rename(&part, dest) {
        let _ = fs::remove_file(&part);
        return Err(error).context(t(
            "не удалось сохранить Android-рантайм",
            "failed to save the Android runtime",
        ));
    }

    progress(Progress::Download {
        file: file.to_string(),
        done,
        total,
    });
    Ok(())
}

fn validate_download(path: &Path, extension: &str) -> Result<()> {
    let mut file = fs::File::open(path)?;
    let mut magic = [0u8; 4];
    let count = file.read(&mut magic)?;

    let valid = if extension == "AppImage" {
        count >= 4 && magic == [0x7f, b'E', b'L', b'F']
    } else if extension == "zip" {
        count >= 2 && magic[0] == b'P' && magic[1] == b'K'
    } else {
        true
    };

    if !valid {
        bail!(
            "{}: {}",
            t(
                "сервер вернул не тот файл",
                "the server returned an unexpected file"
            ),
            path.display()
        );
    }
    Ok(())
}

/// Скачивает AppImage, системный образ и vendor-образ в каталог OpenRun.
pub fn download_runtime(progress: &mut dyn FnMut(Progress)) -> Result<()> {
    let root = runtime_root();
    fs::create_dir_all(&root)?;

    progress(Progress::Status(
        t(
            "Ищу последнюю версию OpenRun Android…",
            "Looking for the latest OpenRun Android version…",
        )
        .to_string(),
    ));
    let appimage = latest_appimage_url()?;
    let system = ota_url(SYSTEM_OTA)?;
    let vendor = ota_url(VENDOR_OTA)?;

    progress(Progress::Status(
        t(
            "Скачиваю OpenRun Android (около 1.1 ГБ)…",
            "Downloading OpenRun Android (about 1.1 GB)…",
        )
        .to_string(),
    ));

    let appimage_path = root.join("OpenRun-Android.AppImage");
    download(&appimage, &appimage_path, "AppImage", progress)?;
    let mut permissions = fs::metadata(&appimage_path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&appimage_path, permissions)?;

    download(
        &system,
        &root.join("system.zip"),
        t("системный образ", "system image"),
        progress,
    )?;
    download(
        &vendor,
        &root.join("vendor.zip"),
        t("vendor-образ", "vendor image"),
        progress,
    )?;

    fs::write(
        root.join("runtime.json"),
        format!("appimage={appimage}\nsystem={system}\nvendor={vendor}\n"),
    )?;
    progress(Progress::Status(
        t("OpenRun Android скачан", "OpenRun Android is downloaded").to_string(),
    ));
    Ok(())
}

/// Внутри Flatpak системного `waydroid` нет, поэтому вызываем его на хосте.
fn waydroid_cmd() -> Command {
    if super::in_flatpak() {
        let mut cmd = Command::new("flatpak-spawn");
        cmd.arg("--host").arg("waydroid");
        cmd
    } else {
        Command::new("waydroid")
    }
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

fn unavailable_error(text: &str) -> bool {
    text.contains("Portal call failed")
        || text.contains("org.freedesktop.DBus.Error.ServiceUnknown")
        || text.contains("command not found")
        || text.contains("No such file")
}

/// Проверяет, что `waydroid` существует и доступен текущему процессу.
pub fn available() -> bool {
    match waydroid_cmd().arg("status").output() {
        Ok(out) => !unavailable_error(&String::from_utf8_lossy(&out.stderr)),
        Err(_) => false,
    }
}

fn parse_state(text: &str, key: &str) -> Session {
    for line in text.lines() {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if !name.trim().eq_ignore_ascii_case(key) {
            continue;
        }
        let value = value.trim().to_ascii_uppercase();
        if value.contains("RUNNING") {
            return Session::Running;
        }
        if value.contains("STOPPED") {
            return Session::Stopped;
        }
        return Session::Unknown;
    }
    Session::Unknown
}

/// Разбирает вывод `waydroid status`.
pub fn parse_status(text: &str) -> Status {
    Status {
        session: parse_state(text, "Session"),
        container: parse_state(text, "Container"),
    }
}

pub fn status() -> Result<Status> {
    let out = waydroid_cmd().arg("status").output().context(t(
        "не удалось запустить Android-рантайм",
        "failed to start the Android runtime",
    ))?;
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    if !out.status.success() && text.trim().is_empty() {
        bail!("{}", output_error(&out));
    }
    Ok(parse_status(&text))
}

/// Разбирает вывод `waydroid app list`.
fn parse_app_list(text: &str) -> Vec<App> {
    let mut apps = Vec::new();
    let mut name: Option<String> = None;
    let mut package: Option<String> = None;

    for line in text.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("Name:") {
            if let (Some(name), Some(package)) = (name.take(), package.take()) {
                apps.push(App { package, name });
            }
            name = Some(value.trim().to_string());
        } else if let Some(value) = line
            .strip_prefix("packageName:")
            .or_else(|| line.strip_prefix("Package:"))
        {
            package = Some(value.trim().to_string());
        }
    }

    if let (Some(name), Some(package)) = (name, package) {
        apps.push(App { package, name });
    }

    apps.retain(|app| !app.package.is_empty());
    apps.sort_by_key(|app| app.name.to_lowercase());
    apps
}

pub fn list_apps() -> Result<Vec<App>> {
    let out = waydroid_cmd().args(["app", "list"]).output().context(t(
        "не удалось запустить Android-рантайм",
        "failed to start the Android runtime",
    ))?;
    if !out.status.success() {
        bail!("{}", output_error(&out));
    }
    Ok(parse_app_list(&String::from_utf8_lossy(&out.stdout)))
}

fn ensure_running() -> Result<()> {
    if !available() {
        bail!(
            "{}",
            t(
                "Android-рантайм OpenRun не найден. Установите компонент Android для OpenRun.",
                "The OpenRun Android runtime was not found. Install the Android component for OpenRun."
            )
        );
    }
    let st = status()?;
    if !st.running() {
        bail!(
            "{}",
            t(
                "Android-рантайм OpenRun не запущен. Сначала запустите Android.",
                "The OpenRun Android runtime is not running. Start Android first."
            )
        );
    }
    Ok(())
}

/// Копирует APK в домашний каталог, чтобы Android-рантайм точно видел файл.
fn stage_apk(path: &Path) -> Result<PathBuf> {
    let path = fs::canonicalize(path).context(t("файл не найден", "file not found"))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("package.apk");
    if !file_name.to_lowercase().ends_with(".apk") {
        bail!("{file_name}: {}", t("это не .apk", "not an .apk file"));
    }
    fs::create_dir_all(staging_dir())?;
    let staged = staging_dir().join(file_name);
    fs::copy(&path, &staged).context(t("не удалось скопировать APK", "failed to copy APK"))?;
    Ok(staged)
}

/// Устанавливает APK. Возвращает запись приложения, если удалось определить
/// новый package name; если приложение уже было установлено — `None`.
pub fn install_file(path: &Path) -> Result<Option<App>> {
    ensure_running()?;
    let staged = stage_apk(path)?;

    let before = list_apps().unwrap_or_default();
    let out = waydroid_cmd()
        .args(["app", "install"])
        .arg(&staged)
        .output()
        .context(t(
            "не удалось запустить Android-рантайм",
            "failed to start the Android runtime",
        ))?;
    let _ = fs::remove_file(&staged);
    if !out.status.success() {
        bail!("{}", output_error(&out));
    }

    let after = list_apps()?;
    Ok(after
        .into_iter()
        .find(|app| !before.iter().any(|old| old.package == app.package)))
}

pub fn launch(package: &str) -> Result<()> {
    if package.trim().is_empty() {
        bail!(
            "{}",
            t("пустое имя Android-пакета", "empty Android package name")
        );
    }
    let mut child = waydroid_cmd()
        .args(["app", "launch", package])
        .stdin(Stdio::null())
        .spawn()
        .context(t(
            "не удалось запустить Android-приложение",
            "failed to launch the Android app",
        ))?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

pub fn remove(package: &str) -> Result<()> {
    let out = waydroid_cmd()
        .args(["app", "remove", package])
        .output()
        .context(t(
            "не удалось запустить Android-рантайм",
            "failed to start the Android runtime",
        ))?;
    if !out.status.success() {
        bail!("{}", output_error(&out));
    }
    Ok(())
}

/// Запускает сессию Android в фоне.
pub fn start_session() -> Result<()> {
    if !available() {
        bail!(
            "{}",
            t(
                "Android-рантайм OpenRun не найден. Установите компонент Android для OpenRun.",
                "The OpenRun Android runtime was not found. Install the Android component for OpenRun."
            )
        );
    }
    let mut child = waydroid_cmd()
        .args(["session", "start"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context(t("не удалось запустить Android", "failed to start Android"))?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

/// Открывает полноэкранный Android UI в отдельном окне OpenRun Android.
pub fn show_full_ui() -> Result<()> {
    if !available() {
        bail!(
            "{}",
            t(
                "Android-рантайм OpenRun не найден. Установите компонент Android для OpenRun.",
                "The OpenRun Android runtime was not found. Install the Android component for OpenRun."
            )
        );
    }
    let mut child = waydroid_cmd()
        .arg("show-full-ui")
        .stdin(Stdio::null())
        .spawn()
        .context(t(
            "не удалось открыть Android UI",
            "failed to open the Android UI",
        ))?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{parse_app_list, parse_status, App, Session, Status};

    #[test]
    fn parses_android_app_list() {
        let text = "\
Name: Files
packageName: com.android.documentsui
categories:
\tandroid.intent.category.LAUNCHER
Name: Calculator
packageName: com.android.calculator2
categories:
\tandroid.intent.category.LAUNCHER
";
        assert_eq!(
            parse_app_list(text),
            vec![
                App {
                    package: "com.android.calculator2".to_string(),
                    name: "Calculator".to_string(),
                },
                App {
                    package: "com.android.documentsui".to_string(),
                    name: "Files".to_string(),
                },
            ]
        );
    }

    #[test]
    fn parses_alternative_package_field() {
        let text = "Name: Example\nPackage: org.example.App\n";
        assert_eq!(
            parse_app_list(text),
            vec![App {
                package: "org.example.App".to_string(),
                name: "Example".to_string(),
            }]
        );
    }

    #[test]
    fn parses_status() {
        assert_eq!(
            parse_status("Session: RUNNING\nContainer: STOPPED\n"),
            Status {
                session: Session::Running,
                container: Session::Stopped,
            }
        );
        assert_eq!(
            parse_status("Session: STOPPED\nContainer: STOPPED\n"),
            Status {
                session: Session::Stopped,
                container: Session::Stopped,
            }
        );
    }
}
