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
    /// The character that ended the previous token, `None` at the start or after a reset.
    last_boundary: Option<char>,
    /// `last_boundary` as it was when the current word started.
    word_prefix: Option<char>,
    /// The whitespace-delimited token being typed contains digits or code punctuation.
    tok_technical: bool,
    /// The previous whitespace-delimited token did.
    prev_technical: bool,
}

/// Keys that are Cyrillic letters on the Russian layout but punctuation on the US one.
const LETTER_PUNCT: &str = "[];',./`";
/// Punctuation that may trail a word without being part of it.
const TRAILING_PUNCT: &[char] = &[',', '.', ';', '\''];
/// Punctuation after which a word may still be auto-fixed.
const FIX_BOUNDARIES: &str = "!?)\"»”.,;";

/// Digits and code punctuation: a token containing them is an identifier, path or number, not prose.
fn is_technical(c: char) -> bool {
    c.is_ascii_digit() || ":_-/\\@#<>=$%&*+|~^".contains(c)
}

impl Engine {
    pub fn new(detector: Detector, leave_alone: LeaveAlone) -> Engine {
        Engine {
            buffer: WordBuffer::new(),
            detector,
            leave_alone,
            enabled: true,
            last_fix: None,
            last_kept: None,
            last_boundary: None,
            word_prefix: None,
            tok_technical: false,
            prev_technical: false,
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
                self.last_boundary = None;
                self.tok_technical = false;
                self.prev_technical = false;
                Output::default()
            }
        }
    }

    fn on_char(&mut self, c: char) -> Output {
        self.last_fix = None;
        let ascii_word = self.buffer.as_str().starts_with(|f: char| f.is_ascii_alphabetic());
        if c.is_alphabetic() || (LETTER_PUNCT.contains(c) && ascii_word) {
            self.last_kept = None;
            if self.buffer.is_empty() {
                self.word_prefix = self.last_boundary;
            }
            self.buffer.push(c);
            return Output::default();
        }
        // `c` ends the word (or is a stray character).
        if is_technical(c) {
            self.tok_technical = true;
        }
        let word = self.buffer.take();
        self.last_kept = None;
        let mut out = Output::default();
        if !word.is_empty() {
            let fixed = if self.enabled && !self.blocked(c) { self.try_fix(&word, c) } else { None };
            match fixed {
                Some(fix) => out = fix,
                None => self.last_kept = Some(Kept { word, boundary: c }),
            }
        }
        if c.is_whitespace() {
            self.prev_technical = self.tok_technical;
            self.tok_technical = false;
        }
        self.last_boundary = Some(c);
        out
    }

    /// Identifiers, paths, numbers and URLs are never fixed, nor words ended by code punctuation.
    fn blocked(&self, boundary: char) -> bool {
        let starts_token = self.word_prefix.is_none_or(char::is_whitespace);
        self.tok_technical
            || (starts_token && self.prev_technical)
            || !(boundary.is_whitespace() || FIX_BOUNDARIES.contains(boundary))
    }

    fn try_fix(&mut self, word: &str, boundary: char) -> Option<Output> {
        // Trailing . , ; ' are ambiguous (ю б ж э on the Russian layout). Judge the word without
        // them, so "it." is never read as the Russian "шею".
        let core = word.trim_end_matches(TRAILING_PUNCT);
        let candidate = if core.is_empty() { word } else { core };
        let Decision::Convert(dir) = self.detector.decide(candidate, self.leave_alone.set()) else {
            return None;
        };
        let trailing = &word[candidate.len()..];
        let text = format!("{}{}{}", convert(candidate, dir), trailing, boundary);
        self.last_fix = Some(Fix {
            core: candidate.to_string(),
            original: word.to_string(),
            boundary: Some(boundary),
            dir,
        });
        Some(Output {
            swallow: false,
            replace: Some(Replace { backspaces: word.chars().count() + 1, text, switch_to: dir }),
        })
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

    #[test]
    fn english_words_before_sentence_punctuation_are_never_fixed() {
        let mut e = engine();
        for w in include_str!("../data/en_words.txt").lines() {
            for p in [",", ".", ";", "'"] {
                assert_eq!(type_str(&mut e, &format!("{w}{p} ")), Output::default(), "{w}{p}");
            }
        }
    }

    #[test]
    fn identifier_and_url_fragments_are_never_fixed() {
        for text in [
            "https://x.y ",
            "http://localhost:8080/a ",
            "cfg_path ",
            "ptr_t ",
            "std::vec::Vec<u8> ",
            "Vec::new() ",
            "<xml> ",
            "10 kb ",
            "my-lb ",
        ] {
            let mut e = engine();
            assert_eq!(type_str(&mut e, text), Output::default(), "{text}");
        }
    }

    #[test]
    fn fixing_resumes_after_a_technical_token_and_a_plain_word() {
        let mut e = engine();
        type_str(&mut e, "10 kb ");
        type_str(&mut e, "ok ");
        assert_eq!(type_str(&mut e, "ghbdtn "), fix(7, "привет ", Direction::EnToRu));
    }

    #[test]
    fn fixes_a_word_in_parentheses_and_quotes() {
        let mut e = engine();
        assert_eq!(type_str(&mut e, "(ghbdtn)"), fix(7, "привет)", Direction::EnToRu));
    }
}
