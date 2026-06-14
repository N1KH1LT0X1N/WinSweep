//! Lightweight i18n scaffolding for WinSweep
//!
//! Supports English (default) and a configurable active locale.
//! Falls back to English when a key is missing.

use once_cell::sync::Lazy;
use parking_lot::RwLock;
use serde::Deserialize;
use std::collections::HashMap;

static TRANSLATIONS: Lazy<RwLock<TranslationSet>> =
    Lazy::new(|| RwLock::new(TranslationSet::load("en")));

/// Translation data loaded at runtime
#[derive(Debug, Clone, Deserialize)]
pub struct TranslationSet {
    #[serde(flatten)]
    pub entries: HashMap<String, String>,
}

impl TranslationSet {
    /// Load translations for a locale, falling back to English for missing keys
    pub fn load(locale: &str) -> Self {
        let mut entries = Self::load_raw("en");
        if locale != "en" {
            let override_entries = Self::load_raw(locale);
            for (k, v) in override_entries {
                entries.insert(k, v);
            }
        }
        Self { entries }
    }

    fn load_raw(locale: &str) -> HashMap<String, String> {
        let yaml = match locale {
            "es" => include_str!("../../../locales/es.yml"),
            _ => include_str!("../../../locales/en.yml"),
        };
        serde_yaml::from_str(yaml).unwrap_or_default()
    }

    /// Get a translation string
    pub fn get(&self, key: &str) -> String {
        self.entries
            .get(key)
            .cloned()
            .unwrap_or_else(|| key.to_string())
    }
}

/// Set the active locale (e.g., "en", "es")
pub fn set_locale(locale: &str) {
    let mut guard = TRANSLATIONS.write();
    *guard = TranslationSet::load(locale);
}

/// Get the current active locale's translation for a key
#[macro_export]
macro_rules! t {
    ($key:expr) => {
        $crate::i18n::translate($key)
    };
}

/// Lookup a translation by key (synchronous, thread-safe)
pub fn translate(key: &str) -> String {
    let guard = TRANSLATIONS.read();
    guard.get(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_english_key_resolution() {
        let ts = TranslationSet::load("en");
        assert_eq!(ts.get("app_name"), "WinSweep");
        assert_eq!(ts.get("nav_dashboard"), "Dashboard");
    }

    #[test]
    fn test_spanish_override() {
        let ts = TranslationSet::load("es");
        assert_eq!(ts.get("app_name"), "WinSweep"); // unchanged
        assert_eq!(ts.get("nav_dashboard"), "Panel"); // overridden
    }

    #[test]
    fn test_fallback_to_key_on_missing() {
        let ts = TranslationSet::load("en");
        assert_eq!(ts.get("nonexistent_key_xyz"), "nonexistent_key_xyz");
    }

    #[test]
    fn test_macro_works() {
        let ts = TranslationSet::load("en");
        assert_eq!(ts.get("app_name"), "WinSweep");
    }
}
