/// Имя приложения и каталогов в ~/.config и ~/.local/share.
pub const APP_NAME: &str = "OpenRun";
pub const APP_DIR: &str = "openrun";

/// true, если OpenRun запущен из своего Flatpak-пакета.
///
/// `/.flatpak-info` есть и у чужих Flatpak-приложений (например, у терминала),
/// поэтому проверяем именно идентификатор OpenRun.
pub fn in_flatpak() -> bool {
    if std::env::var("FLATPAK_ID").is_ok_and(|id| id == crate::APP_ID) {
        return true;
    }
    std::fs::read_to_string("/.flatpak-info")
        .map(|text| text.contains(&format!("name={}", crate::APP_ID)))
        .unwrap_or(false)
}

pub mod config;
pub mod deb;
pub mod flatpak;
pub mod i18n;
pub mod prefix;
pub mod runner;
pub mod runtime;
pub mod shortcut;
pub mod waydroid;
