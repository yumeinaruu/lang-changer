use crate::layout_map::{convert, detect_direction, Direction};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Keep,
    Convert(Direction),
}

const EN_WORDS: &str = include_str!("../data/en_words.txt");
const RU_WORDS: &str = include_str!("../data/ru_words.txt");

/// Words that count as "frequent" for the short-word rule (len < 3).
const SHORT_WORD_RANK: usize = 3000;
/// A typed form scoring below this (mean ln-prob per trigram) is implausible.
const IMPLAUSIBLE_BELOW: f64 = -6.0;
/// Add-k smoothing for trigram counts.
const SMOOTHING: f64 = 0.1;

struct Language {
    words: HashSet<&'static str>,
    frequent: HashSet<&'static str>,
    trigrams: HashMap<[char; 3], f64>,
    total: f64,
    vocab: f64,
}

impl Language {
    fn build(list: &'static str) -> Language {
        let mut words = HashSet::new();
        let mut frequent = HashSet::new();
        let mut counts: HashMap<[char; 3], f64> = HashMap::new();
        let mut total = 0.0;
        for (rank, w) in list.lines().enumerate() {
            words.insert(w);
            if rank < SHORT_WORD_RANK {
                frequent.insert(w);
            }
            for t in trigrams_of(w) {
                *counts.entry(t).or_insert(0.0) += 1.0;
                total += 1.0;
            }
        }
        let vocab = 40.0_f64.powi(3);
        Language { words, frequent, trigrams: counts, total, vocab }
    }

    fn score(&self, word: &str) -> f64 {
        let ts = trigrams_of(word);
        if ts.is_empty() {
            return f64::MIN;
        }
        let denom = self.total + SMOOTHING * self.vocab;
        let sum: f64 = ts
            .iter()
            .map(|t| ((self.trigrams.get(t).copied().unwrap_or(0.0) + SMOOTHING) / denom).ln())
            .sum();
        sum / ts.len() as f64
    }
}

fn trigrams_of(word: &str) -> Vec<[char; 3]> {
    let padded: Vec<char> = "^^"
        .chars()
        .chain(word.to_lowercase().chars())
        .chain(std::iter::once('$'))
        .collect();
    padded.windows(3).map(|w| [w[0], w[1], w[2]]).collect()
}

pub struct Detector {
    en: Language,
    ru: Language,
    threshold: f64,
}

impl Detector {
    pub fn new(threshold: f32) -> Detector {
        Detector {
            en: Language::build(EN_WORDS),
            ru: Language::build(RU_WORDS),
            threshold: threshold as f64,
        }
    }

    pub fn decide(&self, word: &str, leave_alone: &HashSet<String>) -> Decision {
        let lower = word.to_lowercase();
        if word.chars().count() < 2 || leave_alone.contains(&lower) || is_protected(word) {
            return Decision::Keep;
        }
        let dir = detect_direction(word);
        let (typed_lang, target_lang) = match dir {
            Direction::EnToRu => (&self.en, &self.ru),
            Direction::RuToEn => (&self.ru, &self.en),
        };
        let converted = convert(&lower, dir);
        if !is_all_letters(&lower, dir) || !is_all_letters_target(&converted, dir) {
            return Decision::Keep;
        }
        if typed_lang.words.contains(lower.as_str()) {
            return Decision::Keep;
        }
        if word.chars().count() < 3 {
            let known = target_lang.frequent.contains(converted.as_str());
            return if known { Decision::Convert(dir) } else { Decision::Keep };
        }
        if target_lang.words.contains(converted.as_str()) {
            return Decision::Convert(dir);
        }
        let typed_score = typed_lang.score(&lower);
        let conv_score = target_lang.score(&converted);
        if typed_score < IMPLAUSIBLE_BELOW && conv_score - typed_score > self.threshold {
            Decision::Convert(dir)
        } else {
            Decision::Keep
        }
    }
}

/// Identifier-, URL- and number-like tokens are never converted.
fn is_protected(word: &str) -> bool {
    if word.chars().any(|c| c.is_ascii_digit() || matches!(c, '_' | '-' | '/' | '@' | ':')) {
        return true;
    }
    let chars: Vec<char> = word.chars().collect();
    if chars.len() > 2 && chars[1..chars.len() - 1].contains(&'.') {
        return true;
    }
    // CamelCase: a lowercase letter directly followed by an uppercase one.
    chars.windows(2).any(|w| w[0].is_lowercase() && w[1].is_uppercase())
}

fn is_all_letters(word: &str, dir: Direction) -> bool {
    match dir {
        Direction::EnToRu => word.chars().all(|c| c.is_ascii_alphabetic() || "[];',./`".contains(c)),
        Direction::RuToEn => word.chars().all(is_cyrillic),
    }
}

fn is_all_letters_target(word: &str, dir: Direction) -> bool {
    match dir {
        Direction::EnToRu => word.chars().all(is_cyrillic),
        Direction::RuToEn => word.chars().all(|c| c.is_ascii_alphabetic()),
    }
}

fn is_cyrillic(c: char) -> bool {
    ('\u{0400}'..='\u{04FF}').contains(&c)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d() -> Detector {
        Detector::new(1.0)
    }

    fn none() -> HashSet<String> {
        HashSet::new()
    }

    #[test]
    fn converts_wrong_layout_ru() {
        for w in ["ghbdtn", "ntrcn", "gjxtve", "cgfcb,j", "Ghbdtn"] {
            assert_eq!(d().decide(w, &none()), Decision::Convert(Direction::EnToRu), "{w}");
        }
    }

    #[test]
    fn converts_wrong_layout_en() {
        for w in ["руддщ", "цщкдв", "ащк", "Руддщ"] {
            assert_eq!(d().decide(w, &none()), Decision::Convert(Direction::RuToEn), "{w}");
        }
    }

    #[test]
    fn keeps_correct_words() {
        for w in ["hello", "world", "привет", "программа", "Kubernetes", "Москва"] {
            assert_eq!(d().decide(w, &none()), Decision::Keep, "{w}");
        }
    }

    #[test]
    fn keeps_short_words() {
        for w in ["ok", "да", "to", "я", "a", "no"] {
            assert_eq!(d().decide(w, &none()), Decision::Keep, "{w}");
        }
    }

    #[test]
    fn keeps_identifiers() {
        for w in ["getUserName", "foo_bar", "v2.1", "https://x.y", "a-b", "me@x.com"] {
            assert_eq!(d().decide(w, &none()), Decision::Keep, "{w}");
        }
    }

    #[test]
    fn keeps_leave_alone_words() {
        let mut set = HashSet::new();
        set.insert("ghbdtn".to_string());
        assert_eq!(d().decide("ghbdtn", &set), Decision::Keep);
        assert_eq!(d().decide("Ghbdtn", &set), Decision::Keep);
    }

    #[test]
    fn keeps_digits() {
        assert_eq!(d().decide("ghbd1tn", &none()), Decision::Keep);
        assert_eq!(d().decide("123", &none()), Decision::Keep);
    }

    #[test]
    fn keeps_unlisted_technical_words() {
        for w in ["nginx", "kubectl", "github", "docker", "деплой", "бэкенд", "rustc"] {
            assert_eq!(d().decide(w, &none()), Decision::Keep, "{w}");
        }
    }

    #[test]
    fn converts_longer_wrong_layout_words() {
        for w in ["ghjuhfvvf", "gjkmpjdfntkm", "yfcnhjqrf", "ghjtrn"] {
            assert_eq!(d().decide(w, &none()), Decision::Convert(Direction::EnToRu), "{w}");
        }
    }
}
