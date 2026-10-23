use crate::buffer::WordBuffer;
use crate::config::LeaveAlone;
use crate::detector::{Decision, Detector};
use crate::layout_map::{convert, detect_direction, Direction};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyInput {
    /// A printable character as the user sees it (space and punctuation included).
    Char(char),
    Backspace,
    Hotkey,
    /// Anything that makes the buffered word meaningless: navigation, clicks, app switch, Enter/Tab.
    Reset,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replace {
    pub backspaces: usize,
    pub text: String,
    pub switch_to: Direction,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Output {
    /// Do not deliver the key to the focused app.
    pub swallow: bool,
    pub replace: Option<Replace>,
}

/// The last auto- or manual fix, kept so the next key can undo it.
struct Fix {
    /// Word without trailing punctuation, remembered in the leave-alone list on undo.
    core: String,
    /// Everything that was typed for the word, trailing punctuation included.
    original: String,
    boundary: Option<char>,
    dir: Direction,
}

/// A word that was left alone, so the hotkey can still convert it after the boundary.
struct Kept {
    word: String,
    boundary: char,
}

pub struct Engine {
    buffer: WordBuffer,
    detector: Detector,
    leave_alone: LeaveAlone,
    enabled: bool,
    last_fix: Option<Fix>,
    last_kept: Option<Kept>,
}

/// Keys that are Cyrillic letters on the Russian layout but punctuation on the US one.
const LETTER_PUNCT: &str = "[];',./`";
/// Punctuation that may trail a word without being part of it.
const TRAILING_PUNCT: &[char] = &[',', '.', ';', '\''];

impl Engine {
    pub fn new(detector: Detector, leave_alone: LeaveAlone) -> Engine {
        Engine {
            buffer: WordBuffer::new(),
            detector,
            leave_alone,
            enabled: true,
            last_fix: None,
            last_kept: None,
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn on_key(&mut self, input: KeyInput) -> Output {
        match input {
            KeyInput::Char(c) => self.on_char(c),
            KeyInput::Backspace => match self.last_fix.take() {
                Some(fix) => self.undo(fix),
                None => {
                    self.last_kept = None;
                    self.buffer.pop();
                    Output::default()
                }
            },
            KeyInput::Hotkey => self.on_hotkey(),
            KeyInput::Reset => {
                self.buffer.reset();
                self.last_fix = None;
                self.last_kept = None;
                Output::default()
            }
        }
    }

    fn on_char(&mut self, c: char) -> Output {
        self.last_fix = None;
        let ascii_word = self.buffer.as_str().starts_with(|f: char| f.is_ascii_alphabetic());
        if c.is_alphabetic() || (LETTER_PUNCT.contains(c) && ascii_word) {
            self.last_kept = None;
            self.buffer.push(c);
            return Output::default();
        }
        // `c` ends the word.
        let word = self.buffer.take();
        self.last_kept = None;
        if word.is_empty() {
            return Output::default();
        }
        if self.enabled {
            if let Some(out) = self.try_fix(&word, c) {
                return out;
            }
        }
        self.last_kept = Some(Kept { word, boundary: c });
        Output::default()
    }

    fn try_fix(&mut self, word: &str, boundary: char) -> Option<Output> {
        let core = word.trim_end_matches(TRAILING_PUNCT);
        let candidates = if core.len() < word.len() && !core.is_empty() { vec![core, word] } else { vec![word] };
        for candidate in candidates {
            if let Decision::Convert(dir) = self.detector.decide(candidate, self.leave_alone.set()) {
                let trailing = &word[candidate.len()..];
                let text = format!("{}{}{}", convert(candidate, dir), trailing, boundary);
                self.last_fix = Some(Fix {
                    core: candidate.to_string(),
                    original: word.to_string(),
                    boundary: Some(boundary),
                    dir,
                });
                return Some(Output {
                    swallow: false,
                    replace: Some(Replace { backspaces: word.chars().count() + 1, text, switch_to: dir }),
                });
            }
        }
        None
    }

    fn on_hotkey(&mut self) -> Output {
        if let Some(fix) = self.last_fix.take() {
            return self.undo(fix);
        }
        let (word, boundary) = if !self.buffer.is_empty() {
            (self.buffer.take(), None)
        } else if let Some(kept) = self.last_kept.take() {
            (kept.word, Some(kept.boundary))
        } else {
            return Output { swallow: true, replace: None };
        };
        let dir = detect_direction(&word);
        let text = format!("{}{}", convert(&word, dir), boundary.map(String::from).unwrap_or_default());
        let backspaces = word.chars().count() + boundary.map_or(0, |_| 1);
        self.last_fix = Some(Fix { core: word.clone(), original: word, boundary, dir });
        Output { swallow: true, replace: Some(Replace { backspaces, text, switch_to: dir }) }
    }

    fn undo(&mut self, fix: Fix) -> Output {
        self.leave_alone.add(&fix.core);
        let back = match fix.dir {
            Direction::EnToRu => Direction::RuToEn,
            Direction::RuToEn => Direction::EnToRu,
        };
        let boundary = fix.boundary.map(String::from).unwrap_or_default();
        Output {
            swallow: true,
            replace: Some(Replace {
                backspaces: fix.original.chars().count() + fix.boundary.map_or(0, |_| 1),
                text: format!("{}{}", fix.original, boundary),
                switch_to: back,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use KeyInput::*;

    fn engine() -> Engine {
        let dir = std::env::temp_dir().join(format!("lang-changer-engine-{}-{:?}", std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_dir_all(&dir);
        Engine::new(Detector::new(1.0), LeaveAlone::load(&dir))
    }

    fn type_str(e: &mut Engine, s: &str) -> Output {
        let mut last = Output::default();
        for c in s.chars() {
            last = e.on_key(Char(c));
        }
        last
    }

    fn fix(backspaces: usize, text: &str, switch_to: Direction) -> Output {
        Output { swallow: false, replace: Some(Replace { backspaces, text: text.into(), switch_to }) }
    }

    #[test]
    fn fixes_wrong_layout_word_at_space() {
        let mut e = engine();
        assert_eq!(type_str(&mut e, "ghbdtn "), fix(7, "привет ", Direction::EnToRu));
    }

    #[test]
    fn preserves_case_when_fixing() {
        let mut e = engine();
        assert_eq!(type_str(&mut e, "Ghbdtn "), fix(7, "Привет ", Direction::EnToRu));
    }

    #[test]
    fn fixes_russian_typed_on_english_layout_back() {
        let mut e = engine();
        assert_eq!(type_str(&mut e, "руддщ "), fix(6, "hello ", Direction::RuToEn));
    }

    #[test]
    fn keeps_correct_word() {
        let mut e = engine();
        assert_eq!(type_str(&mut e, "hello "), Output::default());
    }

    #[test]
    fn fixes_before_punctuation_boundary() {
        let mut e = engine();
        assert_eq!(type_str(&mut e, "ghbdtn!"), fix(7, "привет!", Direction::EnToRu));
        assert_eq!(e.on_key(Char(' ')), Output::default());
    }

    #[test]
    fn trailing_comma_stays_after_fixed_word() {
        let mut e = engine();
        assert_eq!(type_str(&mut e, "ghbdtn, "), fix(8, "привет, ", Direction::EnToRu));
    }

    #[test]
    fn russian_letters_typed_as_punctuation_stay_inside_word() {
        let mut e = engine();
        // "j,hfpjv" is "образом" on the Russian layout
        assert_eq!(type_str(&mut e, "j,hfpjv "), fix(8, "образом ", Direction::EnToRu));
    }

    #[test]
    fn backspace_right_after_fix_restores_original_and_is_swallowed() {
        let mut e = engine();
        type_str(&mut e, "ghbdtn ");
        let out = e.on_key(Backspace);
        assert_eq!(out, Output { swallow: true, replace: Some(Replace { backspaces: 7, text: "ghbdtn ".into(), switch_to: Direction::RuToEn }) });
        // the word is now remembered and never fixed again
        assert_eq!(type_str(&mut e, "ghbdtn "), Output::default());
    }

    #[test]
    fn hotkey_right_after_fix_undoes_it() {
        let mut e = engine();
        type_str(&mut e, "Ghbdtn ");
        let out = e.on_key(Hotkey);
        assert_eq!(out, Output { swallow: true, replace: Some(Replace { backspaces: 7, text: "Ghbdtn ".into(), switch_to: Direction::RuToEn }) });
    }

    #[test]
    fn other_key_after_fix_forgets_undo() {
        let mut e = engine();
        type_str(&mut e, "ghbdtn ");
        e.on_key(Char('в'));
        assert_eq!(e.on_key(Backspace), Output::default());
    }

    #[test]
    fn hotkey_converts_buffered_word_without_boundary() {
        let mut e = engine();
        type_str(&mut e, "nginx");
        let out = e.on_key(Hotkey);
        assert_eq!(out, Output { swallow: true, replace: Some(Replace { backspaces: 5, text: "тпштч".into(), switch_to: Direction::EnToRu }) });
    }

    #[test]
    fn hotkey_converts_last_kept_word_after_boundary() {
        let mut e = engine();
        type_str(&mut e, "nginx ");
        let out = e.on_key(Hotkey);
        assert_eq!(out, Output { swallow: true, replace: Some(Replace { backspaces: 6, text: "тпштч ".into(), switch_to: Direction::EnToRu }) });
    }

    #[test]
    fn hotkey_with_nothing_to_convert_is_swallowed_so_the_selection_path_can_run() {
        let mut e = engine();
        assert_eq!(e.on_key(Hotkey), Output { swallow: true, replace: None });
    }

    #[test]
    fn disabled_engine_does_not_auto_fix_but_hotkey_works() {
        let mut e = engine();
        e.set_enabled(false);
        assert_eq!(type_str(&mut e, "ghbdtn "), Output::default());
        let out = e.on_key(Hotkey);
        assert!(out.swallow);
        assert_eq!(out.replace.unwrap().text, "привет ");
    }

    #[test]
    fn reset_drops_buffered_word() {
        let mut e = engine();
        type_str(&mut e, "ghbd");
        e.on_key(Reset);
        // without the reset this would complete "ghbdtn" and be fixed
        assert_eq!(type_str(&mut e, "tn "), Output::default());
    }

    #[test]
    fn backspace_edits_buffered_word() {
        let mut e = engine();
        type_str(&mut e, "ghbdtnx");
        e.on_key(Backspace);
        assert_eq!(e.on_key(Char(' ')), fix(7, "привет ", Direction::EnToRu));
    }

    #[test]
    fn identifiers_are_never_fixed() {
        let mut e = engine();
        assert_eq!(type_str(&mut e, "getUserName "), Output::default());
        assert_eq!(type_str(&mut e, "foo_bar "), Output::default());
    }
}
