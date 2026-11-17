use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::i18n::Lang;
use super::prefix::{Prefix, Runner};
use crate::core::APP_DIR;

/// Windows-программа, запущенная через менеджер: её можно снова запустить
/// из приложения или через созданный ярлык.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppEntry {
    pub name: String,
    pub exe: PathBuf,
    pub prefix_name: String,
    pub prefix: PathBuf,
    pub runner: Runner,
}

impl AppEntry {
    /// Восстанавливает префикс, в котором работает программа.
    pub fn prefix(&self) -> Prefix {
        Prefix {
            name: self.prefix_name.clone(),
            path: self.prefix.clone(),
            runner: self.runner.clone(),
            env: Default::default(),
        }
    }
}

/// Тема оформления интерфейса.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub prefixes: Vec<Prefix>,
    #[serde(default)]
    pub apps: Vec<AppEntry>,
    #[serde(default)]
    pub language: Lang,
    #[serde(default)]
    pub theme: Theme,
}

impl Config {
    pub fn path() -> Result<PathBuf> {
        let dir = dirs::config_dir()
            .context("не найден каталог настроек пользователя")?
            .join(crate::core::APP_DIR);
        Ok(dir.join("config.toml"))
    }

    /// Загружает конфиг; при любой ошибке возвращает пустой конфиг, чтобы приложение не падало.
    pub fn load() -> Self {
        Self::path()
            .ok()
            .and_then(|p| fs::read_to_string(p).ok())
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, toml::to_string_pretty(self)?)?;
        Ok(())
    }
}

/// Переносит настройки и данные прежних версий (`wine-manager`) в каталоги OpenRun.
/// Нужен один раз при обновлении: старые префиксы и скачанный рантайм остаются на месте.
pub fn migrate_legacy() {
    let data = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    let config = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));

    let old_data = data.join("wine-manager");
    let new_data = data.join(APP_DIR);
    if old_data.is_dir() && !new_data.exists() {
        let _ = fs::rename(&old_data, &new_data);
    }

    let old_cfg_dir = config.join("wine-manager");
    let new_cfg_dir = config.join(APP_DIR);
    let new_cfg = new_cfg_dir.join("config.toml");
    if !new_cfg.exists() {
        if let Ok(text) = fs::read_to_string(old_cfg_dir.join("config.toml")) {
            // Пути префиксов внутри конфига тоже переносим на новый каталог.
            let text = text.replace("wine-manager", APP_DIR);
            if fs::create_dir_all(&new_cfg_dir).is_ok() {
                let _ = fs::write(&new_cfg, text);
            }
        }
    }
    let _ = fs::remove_dir_all(&old_cfg_dir);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::prefix::Runner;
    use std::path::Path;

    #[test]
    fn parses_language_and_theme() {
        let cfg: Config = toml::from_str("language = \"en\"\ntheme = \"dark\"\n").unwrap();
        assert_eq!(cfg.language, Lang::En);
        assert_eq!(cfg.theme, Theme::Dark);
    }

    #[test]
    fn config_roundtrip() {
        let mut cfg = Config::default();
        cfg.prefixes.push(Prefix::new("Офис", Path::new("/tmp/wm")));
        cfg.prefixes[0].runner = Runner::Proton {
            path: PathBuf::from("/opt/proton"),
        };
        let text = toml::to_string_pretty(&cfg).unwrap();
        let back: Config = toml::from_str(&text).unwrap();
        assert_eq!(back.prefixes, cfg.prefixes);
    }
}
