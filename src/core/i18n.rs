//! Простейшая локализация интерфейса: русский и английский.
//!
//! Язык хранится в атомарной переменной, чтобы его видели и фоновые потоки
//! (например, поток скачивания рантайма, который присылает статусы).

use std::sync::atomic::{AtomicU8, Ordering};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    #[default]
    Ru,
    En,
}

impl Lang {
    fn from_u8(value: u8) -> Self {
        if value == 1 {
            Lang::En
        } else {
            Lang::Ru
        }
    }
}

static LANG: AtomicU8 = AtomicU8::new(0);

pub fn current() -> Lang {
    Lang::from_u8(LANG.load(Ordering::Relaxed))
}

pub fn set(lang: Lang) {
    LANG.store(lang as u8, Ordering::Relaxed);
}

/// Возвращает строку на текущем языке.
pub fn t(ru: &'static str, en: &'static str) -> &'static str {
    match current() {
        Lang::Ru => ru,
        Lang::En => en,
    }
}
