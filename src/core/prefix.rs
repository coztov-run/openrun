use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Runner {
    System,
    Proton { path: PathBuf },
}


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Prefix {
    pub name: String,
    pub path: PathBuf,
    pub runner: Runner,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

impl Prefix {
    pub fn new(name: &str, root: &Path) -> Self {
        Self {
            name: name.to_string(),
            path: root.join(slug(name)),
            runner: Runner::System,
            env: BTreeMap::new(),
        }
    }

    pub fn default_root() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(crate::core::APP_DIR)
            .join("prefixes")
    }
}

pub fn slug(name: &str) -> String {
    let s: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c.to_lowercase().next().unwrap_or(c)
            } else {
                '-'
            }
        })
        .collect();
    let s = s.trim_matches('-').to_string();
    if s.is_empty() {
        "prefix".to_string()
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_keeps_letters_and_replaces_spaces() {
        assert_eq!(slug("Офис 2019"), "офис-2019");
        assert_eq!(slug("   "), "prefix");
        assert_eq!(slug(r"a/b\c"), "a-b-c");
    }
}
