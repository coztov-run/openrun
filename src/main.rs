mod core;
mod ui;

use std::path::{Path, PathBuf};

use gtk4::gio;
use gtk4::glib;
use libadwaita as adw;
use libadwaita::prelude::*;

pub const APP_ID: &str = "io.github.coztov_run.OpenRun";

fn main() -> glib::ExitCode {
    core::config::migrate_legacy();
    glib::set_application_name(core::APP_NAME);
    let app = adw::Application::builder().application_id(APP_ID).build();

    // Файловый менеджер запускает OpenRun с путём к файлу (Exec=… %F).
    // HANDLES_COMMAND_LINE нужен, чтобы аргументы доходили и до уже запущенного
    // экземпляра: второй процесс просто передаёт файл первому.
    app.set_flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE);
    app.connect_command_line(|app, cmdline| {
        // Так ярлыки программ, созданные внутри Flatpak, запускают OpenRun
        // с нужной записью: `flatpak run … --launch-app "Название"`.
        let arguments: Vec<String> = cmdline
            .arguments()
            .into_iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        if let Some(pos) = arguments.iter().position(|arg| arg == "--launch-app") {
            if let Some(name) = arguments.get(pos + 1).filter(|name| !name.is_empty()) {
                ui::open_app(app, name);
                return 0;
            }
        }
        if let Some(pos) = arguments.iter().position(|arg| arg == "--launch-android") {
            if let Some(package) = arguments.get(pos + 1).filter(|name| !name.is_empty()) {
                ui::open_android(app, package);
                return 0;
            }
        }

        let files = files_from_command_line(cmdline);
        ui::open(app, files);
        0
    });
    // Запасной путь для окружений, где command-line не приходит.
    app.connect_activate(|app| ui::open(app, Vec::new()));

    app.run()
}

/// Берёт из командной строки только поддерживаемые файлы и делает пути абсолютными.
fn files_from_command_line(cmdline: &gio::ApplicationCommandLine) -> Vec<PathBuf> {
    let cwd = cmdline.cwd();
    cmdline
        .arguments()
        .into_iter()
        .skip(1)
        .map(PathBuf::from)
        .filter(|path| is_supported_file(path))
        .map(|path| match (&cwd, path.is_relative()) {
            (Some(cwd), true) => cwd.join(path),
            _ => path,
        })
        .collect()
}

fn is_supported_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| {
            let ext = ext.to_ascii_lowercase();
            matches!(ext.as_str(), "exe" | "msi" | "deb" | "flatpak" | "apk")
        })
}

#[cfg(test)]
mod tests {
    use super::is_supported_file;
    use std::path::Path;

    #[test]
    fn recognises_supported_extensions() {
        assert!(is_supported_file(Path::new("setup.exe")));
        assert!(is_supported_file(Path::new("setup.EXE")));
        assert!(is_supported_file(Path::new("package.msi")));
        assert!(is_supported_file(Path::new("package.deb")));
        assert!(is_supported_file(Path::new("package.flatpak")));
        assert!(is_supported_file(Path::new("application.apk")));
    }

    #[test]
    fn ignores_other_files() {
        assert!(!is_supported_file(Path::new("notes.txt")));
        assert!(!is_supported_file(Path::new("archive.tar.gz")));
        assert!(!is_supported_file(Path::new("README")));
    }
}
