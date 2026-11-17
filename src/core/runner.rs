use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};

use super::i18n::t;
use super::prefix::{Prefix, Runner};
use super::runtime;

fn steam_root() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".steam/steam")
}

pub fn base_command(prefix: &Prefix) -> Result<Command> {
    std::fs::create_dir_all(&prefix.path)
        .with_context(|| format!("не удалось создать {}", prefix.path.display()))?;

    let mut cmd = match &prefix.runner {
        Runner::System => {
            // Встроенный рантайм OpenRun; системный рантайм — только как запасной вариант.
            let mut c = Command::new(runtime::wine_bin());
            c.env("WINEPREFIX", &prefix.path);
            c.env("WINEDEBUG", "-all");
            // Не предлагать Mono/Gecko и не плодить лишние ярлыки — их создаёт сам OpenRun.
            c.env("WINEDLLOVERRIDES", "mscoree,mshtml=;winemenubuilder.exe=d");
            // Быстрая синхронизация потоков; если ядро не поддерживает, рантайм откатится сам.
            c.env("WINEFSYNC", "1");
            c.env("WINEESYNC", "1");
            c
        }
        Runner::Proton { path } => {
            let mut c = Command::new(path.join("proton"));
            c.arg("run");
            c.env("STEAM_COMPAT_DATA_PATH", &prefix.path);
            c.env("STEAM_COMPAT_CLIENT_INSTALL_PATH", steam_root());
            c
        }
    };
    cmd.envs(&prefix.env);
    cmd.stdin(Stdio::null());
    Ok(cmd)
}

pub fn create_prefix(prefix: &Prefix) -> Result<()> {
    let mut cmd = base_command(prefix)?;
    cmd.args(["wineboot", "-u"]);
    let out = cmd
        .output()
        .context(t("не удалось запустить OpenRun/Proton", "failed to launch OpenRun/Proton"))?;
    if !out.status.success() {
        bail!("{}", t("подготовка префикса OpenRun завершилась с ошибкой", "OpenRun prefix preparation failed"));
    }
    Ok(())
}

pub fn ensure_prefix(prefix: &Prefix) -> Result<()> {
    let ready = prefix.path.join("system.reg").exists()
        || prefix.path.join("pfx/system.reg").exists();
    if ready {
        Ok(())
    } else {
        create_prefix(prefix)
    }
}

pub fn run_exe(prefix: &Prefix, exe: &Path) -> Result<()> {
    if !exe.exists() {
        bail!("{}: {}", t("файл не найден", "file not found"), exe.display());
    }
    let mut cmd = base_command(prefix)?;
    let is_msi = exe
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("msi"));
    if is_msi {
        cmd.args(["msiexec", "/i"]);
    }
    cmd.arg(exe);
    if let Some(dir) = exe.parent() {
        cmd.current_dir(dir);
    }
    let mut child = cmd
        .spawn()
        .context(t("не удалось запустить OpenRun/Proton", "failed to launch OpenRun/Proton"))?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

pub fn run_winecfg(prefix: &Prefix) -> Result<()> {
    let mut cmd = base_command(prefix)?;
    cmd.arg("winecfg");
    cmd.spawn().context(t("не удалось открыть настройки OpenRun", "failed to open OpenRun settings"))?;
    Ok(())
}
