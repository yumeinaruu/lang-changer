#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    EnToRu,
    RuToEn,
}

// Standard macOS "Russian" (ЙЦУКЕН) against US QWERTY, key for key.
const EN: &str = "qwertyuiop[]asdfghjkl;'zxcvbnm,./`QWERTYUIOP{}ASDFGHJKL:\"ZXCVBNM<>?~";
const RU: &str = "йцукенгшщзхъфывапролджэячсмитьбю.ёЙЦУКЕНГШЩЗХЪФЫВАПРОЛДЖЭЯЧСМИТЬБЮ,Ё";

pub fn convert(text: &str, dir: Direction) -> String {
    let (from, to) = match dir {
        Direction::EnToRu => (EN, RU),
        Direction::RuToEn => (RU, EN),
    };
    text.chars()
        .map(|c| match from.chars().position(|f| f == c) {
            Some(i) => to.chars().nth(i).unwrap_or(c),
            None => c,
        })
        .collect()
}

pub fn detect_direction(text: &str) -> Direction {
    let latin = text.chars().filter(|c| c.is_ascii_alphabetic()).count();
    let cyrillic = text
        .chars()
        .filter(|c| ('\u{0400}'..='\u{04FF}').contains(c))
        .count();
    if cyrillic > latin {
        Direction::RuToEn
    } else {
        Direction::EnToRu
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_en_to_ru() {
        assert_eq!(convert("ghbdtn", Direction::EnToRu), "привет");
    }

    #[test]
    fn converts_ru_to_en() {
        assert_eq!(convert("руддщ", Direction::RuToEn), "hello");
    }

    #[test]
    fn preserves_case() {
        assert_eq!(convert("Ghbdtn", Direction::EnToRu), "Привет");
    }

    #[test]
    fn maps_punctuation_keys() {
        assert_eq!(convert("[];',.", Direction::EnToRu), "хъжэбю");
    }

    #[test]
    fn roundtrip_letters() {
        let en = "qwertyuiopasdfghjklzxcvbnmQWERTYUIOPASDFGHJKLZXCVBNM";
        let ru = convert(en, Direction::EnToRu);
        assert_ne!(ru, en);
        assert_eq!(convert(&ru, Direction::RuToEn), en);
    }

    #[test]
    fn detect_direction_mixed() {
        assert_eq!(detect_direction("привет hi"), Direction::RuToEn);
        assert_eq!(detect_direction("ghbdtn при"), Direction::EnToRu);
    }
}
