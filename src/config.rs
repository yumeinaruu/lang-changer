use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

const DEFAULT_THRESHOLD: f32 = 1.0;
const DEFAULT_HOTKEY: &str = "ctrl+f";

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Hotkey {
    pub ctrl: bool,
    pub alt: bool,
    pub cmd: bool,
    pub shift: bool,
    pub key: String,
}

impl Hotkey {
    /// Parses `"ctrl+f"`, `"cmd+shift+space"`: modifiers then exactly one key.
    pub fn parse(s: &str) -> Option<Hotkey> {
        let mut hk = Hotkey::default();
        for part in s.split('+').map(|p| p.trim().to_lowercase()) {
            match part.as_str() {
                "ctrl" | "control" => hk.ctrl = true,
                "alt" | "option" | "opt" => hk.alt = true,
                "cmd" | "command" => hk.cmd = true,
                "shift" => hk.shift = true,
                "" => return None,
                key if hk.key.is_empty() => hk.key = key.to_string(),
                _ => return None,
            }
        }
        if hk.key.is_empty() { None } else { Some(hk) }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub hotkey: Hotkey,
    pub threshold: f32,
    pub enabled: bool,
}

#[derive(Deserialize)]
struct RawConfig {
    hotkey: Option<String>,
    threshold: Option<f32>,
    enabled: Option<bool>,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            hotkey: Hotkey::parse(DEFAULT_HOTKEY).expect("default hotkey is valid"),
            threshold: DEFAULT_THRESHOLD,
            enabled: true,
        }
    }
}

impl Config {
    /// Reads `<dir>/config.toml`; a missing or invalid file yields defaults, field by field.
    pub fn load(dir: &Path) -> Config {
        let default = Config::default();
        let raw: RawConfig = match fs::read_to_string(dir.join("config.toml"))
            .ok()
            .and_then(|text| toml::from_str(&text).ok())
        {
            Some(raw) => raw,
            None => return default,
        };
        Config {
            hotkey: raw.hotkey.as_deref().and_then(Hotkey::parse).unwrap_or(default.hotkey),
            threshold: raw.threshold.unwrap_or(default.threshold),
            enabled: raw.enabled.unwrap_or(default.enabled),
        }
    }
}

/// Words the user undid an auto-fix for; persisted one lowercase word per line.
pub struct LeaveAlone {
    path: PathBuf,
    words: HashSet<String>,
}

impl LeaveAlone {
    pub fn load(dir: &Path) -> LeaveAlone {
        let path = dir.join("leave_alone.txt");
        let words = fs::read_to_string(&path)
            .map(|text| text.lines().map(str::to_string).filter(|w| !w.is_empty()).collect())
            .unwrap_or_default();
        LeaveAlone { path, words }
    }

    pub fn add(&mut self, word: &str) {
        let word = word.to_lowercase();
        if !self.words.insert(word.clone()) {
            return;
        }
        if let Some(parent) = self.path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(&self.path) {
            let _ = writeln!(f, "{word}");
        }
    }

    pub fn set(&self) -> &HashSet<String> {
        &self.words
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lang-changer-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn ctrl_f() -> Hotkey {
        Hotkey { ctrl: true, alt: false, cmd: false, shift: false, key: "f".into() }
    }

    #[test]
    fn defaults_when_missing() {
        let dir = temp_dir("missing");
        let c = Config::load(&dir);
        assert_eq!(c.hotkey, ctrl_f());
        assert_eq!(c.threshold, 1.0);
        assert!(c.enabled);
    }

    #[test]
    fn invalid_toml_falls_back_to_defaults() {
        let dir = temp_dir("invalid");
        fs::write(dir.join("config.toml"), "this is = not [valid").unwrap();
        let c = Config::load(&dir);
        assert_eq!(c.hotkey, ctrl_f());
        assert!(c.enabled);
    }

    #[test]
    fn reads_custom_hotkey_and_threshold() {
        let dir = temp_dir("custom");
        fs::write(dir.join("config.toml"), "hotkey = \"cmd+shift+space\"\nthreshold = 2.5\n").unwrap();
        let c = Config::load(&dir);
        assert_eq!(c.hotkey, Hotkey { ctrl: false, alt: false, cmd: true, shift: true, key: "space".into() });
        assert_eq!(c.threshold, 2.5);
    }

    #[test]
    fn bad_hotkey_string_falls_back_to_default() {
        assert_eq!(Hotkey::parse(""), None);
        let dir = temp_dir("badhotkey");
        fs::write(dir.join("config.toml"), "hotkey = \"+++\"\n").unwrap();
        assert_eq!(Config::load(&dir).hotkey, ctrl_f());
    }

    #[test]
    fn leave_alone_persists_across_loads() {
        let dir = temp_dir("leave");
        let mut l = LeaveAlone::load(&dir);
        assert!(l.set().is_empty());
        l.add("Ghbdtn");
        assert!(l.set().contains("ghbdtn"));
        let again = LeaveAlone::load(&dir);
        assert!(again.set().contains("ghbdtn"));
    }
}
