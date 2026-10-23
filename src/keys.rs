use crate::config::Hotkey;
use crate::engine::KeyInput;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mods {
    pub ctrl: bool,
    pub alt: bool,
    pub cmd: bool,
    pub shift: bool,
}

const KEYCODE_BACKSPACE: u16 = 51;

/// ANSI virtual keycodes; they do not change with the active input source.
pub fn keycode_for(name: &str) -> Option<u16> {
    let code = match name {
        "a" => 0, "s" => 1, "d" => 2, "f" => 3, "h" => 4, "g" => 5, "z" => 6, "x" => 7,
        "c" => 8, "v" => 9, "b" => 11, "q" => 12, "w" => 13, "e" => 14, "r" => 15, "y" => 16,
        "t" => 17, "o" => 31, "u" => 32, "i" => 34, "p" => 35, "l" => 37, "j" => 38, "k" => 40,
        "n" => 45, "m" => 46,
        "1" => 18, "2" => 19, "3" => 20, "4" => 21, "6" => 22, "5" => 23, "9" => 25, "7" => 26,
        "8" => 28, "0" => 29,
        "space" => 49,
        _ => return None,
    };
    Some(code)
}

/// Turns a key-down into what the engine cares about. `text` is the character(s) the key produces.
pub fn classify(keycode: u16, mods: Mods, text: &str, hotkey: &Hotkey) -> KeyInput {
    let hotkey_mods = Mods { ctrl: hotkey.ctrl, alt: hotkey.alt, cmd: hotkey.cmd, shift: hotkey.shift };
    if keycode_for(&hotkey.key) == Some(keycode) && mods == hotkey_mods {
        return KeyInput::Hotkey;
    }
    if mods.ctrl || mods.alt || mods.cmd {
        return KeyInput::Reset;
    }
    if keycode == KEYCODE_BACKSPACE {
        return KeyInput::Backspace;
    }
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if !c.is_control() => KeyInput::Char(c),
        _ => KeyInput::Reset, // Return, Tab, Escape, arrows, function keys
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Hotkey;
    use crate::engine::KeyInput;

    fn ctrl_f() -> Hotkey {
        Hotkey::parse("ctrl+f").unwrap()
    }

    const NONE: Mods = Mods { ctrl: false, alt: false, cmd: false, shift: false };

    #[test]
    fn hotkey_matches_by_keycode_not_by_text() {
        // Ctrl+F produces a control character (or Cyrillic "а" on the RU layout); keycode 3 is what counts.
        let ctrl = Mods { ctrl: true, ..NONE };
        assert_eq!(classify(3, ctrl, "\u{6}", &ctrl_f()), KeyInput::Hotkey);
        assert_eq!(classify(3, ctrl, "а", &ctrl_f()), KeyInput::Hotkey);
    }

    #[test]
    fn hotkey_needs_exact_modifiers() {
        let ctrl_shift = Mods { ctrl: true, shift: true, ..NONE };
        assert_eq!(classify(3, ctrl_shift, "F", &ctrl_f()), KeyInput::Reset);
        assert_eq!(classify(3, NONE, "f", &ctrl_f()), KeyInput::Char('f'));
    }

    #[test]
    fn plain_and_shifted_characters_pass_through() {
        assert_eq!(classify(0, NONE, "a", &ctrl_f()), KeyInput::Char('a'));
        assert_eq!(classify(0, Mods { shift: true, ..NONE }, "A", &ctrl_f()), KeyInput::Char('A'));
        assert_eq!(classify(49, NONE, " ", &ctrl_f()), KeyInput::Char(' '));
    }

    #[test]
    fn command_control_option_combos_reset() {
        assert_eq!(classify(8, Mods { cmd: true, ..NONE }, "c", &ctrl_f()), KeyInput::Reset);
        assert_eq!(classify(0, Mods { ctrl: true, ..NONE }, "\u{1}", &ctrl_f()), KeyInput::Reset);
        assert_eq!(classify(0, Mods { alt: true, ..NONE }, "å", &ctrl_f()), KeyInput::Reset);
    }

    #[test]
    fn special_keys() {
        assert_eq!(classify(51, NONE, "\u{7f}", &ctrl_f()), KeyInput::Backspace);
        assert_eq!(classify(36, NONE, "\r", &ctrl_f()), KeyInput::Reset); // Return
        assert_eq!(classify(48, NONE, "\t", &ctrl_f()), KeyInput::Reset); // Tab
        assert_eq!(classify(123, NONE, "", &ctrl_f()), KeyInput::Reset); // arrow
        assert_eq!(classify(53, NONE, "\u{1b}", &ctrl_f()), KeyInput::Reset); // Escape
    }

    #[test]
    fn keycode_lookup() {
        assert_eq!(keycode_for("f"), Some(3));
        assert_eq!(keycode_for("space"), Some(49));
        assert_eq!(keycode_for("nonsense"), None);
    }
}
