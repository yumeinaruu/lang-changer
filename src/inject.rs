use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation, EventField};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

/// Stamped on every synthetic event so the event tap can ignore its own output.
const OWN_EVENT_MARKER: i64 = 0x4C43_4843; // "LCHC"
const KEYCODE_BACKSPACE: u16 = 51;
/// `CGEventKeyboardSetUnicodeString` is reliable up to ~20 UTF-16 units per event.
const CHUNK_CHARS: usize = 10;

pub fn is_own_event(event: &CGEvent) -> bool {
    event.get_integer_value_field(EventField::EVENT_SOURCE_USER_DATA) == OWN_EVENT_MARKER
}

/// Deletes `backspaces` characters before the cursor, then types `text`.
pub fn replace(backspaces: usize, text: &str) {
    let Ok(source) = CGEventSource::new(CGEventSourceStateID::Private) else {
        return;
    };
    for _ in 0..backspaces {
        post_key(&source, KEYCODE_BACKSPACE, None);
    }
    let chars: Vec<char> = text.chars().collect();
    for chunk in chars.chunks(CHUNK_CHARS) {
        let s: String = chunk.iter().collect();
        post_key(&source, 0, Some(&s));
    }
}

/// Presses Cmd plus `keycode` (Cmd-C, Cmd-V).
pub fn command_key(keycode: u16) {
    let Ok(source) = CGEventSource::new(CGEventSourceStateID::Private) else {
        return;
    };
    for down in [true, false] {
        if let Ok(event) = CGEvent::new_keyboard_event(source.clone(), keycode, down) {
            event.set_flags(CGEventFlags::CGEventFlagCommand);
            event.set_integer_value_field(EventField::EVENT_SOURCE_USER_DATA, OWN_EVENT_MARKER);
            event.post(CGEventTapLocation::HID);
        }
    }
}

fn post_key(source: &CGEventSource, keycode: u16, text: Option<&str>) {
    for down in [true, false] {
        if let Ok(event) = CGEvent::new_keyboard_event(source.clone(), keycode, down) {
            if let Some(t) = text {
                event.set_string(t);
            }
            event.set_integer_value_field(EventField::EVENT_SOURCE_USER_DATA, OWN_EVENT_MARKER);
            event.post(CGEventTapLocation::HID);
        }
    }
}
