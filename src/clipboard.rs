use crate::inject;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_app_kit::{NSPasteboard, NSPasteboardItem, NSPasteboardType, NSPasteboardTypeString};
use objc2_foundation::{NSArray, NSData, NSString};
use std::thread;
use std::time::{Duration, Instant};

const KEYCODE_C: u16 = 8;
const KEYCODE_V: u16 = 9;
/// How long to wait for the focused app to put its selection on the pasteboard.
const COPY_TIMEOUT: Duration = Duration::from_millis(250);
/// Time the focused app needs to read the pasteboard after Cmd-V, before we restore it.
const PASTE_SETTLE: Duration = Duration::from_millis(150);

/// Every flavor of every item on the pasteboard, as raw data.
pub struct Saved(Vec<Vec<(Retained<NSPasteboardType>, Retained<NSData>)>>);

pub fn save() -> Saved {
    let pb = NSPasteboard::generalPasteboard();
    let mut items = Vec::new();
    if let Some(list) = pb.pasteboardItems() {
        for item in list.iter() {
            let flavors = item
                .types()
                .iter()
                .filter_map(|t| item.dataForType(&t).map(|data| (t, data)))
                .collect();
            items.push(flavors);
        }
    }
    Saved(items)
}

pub fn restore(saved: Saved) {
    let pb = NSPasteboard::generalPasteboard();
    pb.clearContents();
    let items: Vec<_> = saved
        .0
        .iter()
        .map(|flavors| {
            let item = NSPasteboardItem::new();
            for (kind, data) in flavors {
                item.setData_forType(data, kind);
            }
            ProtocolObject::from_retained(item)
        })
        .collect();
    if !items.is_empty() {
        pb.writeObjects(&NSArray::from_retained_slice(&items));
    }
}

/// Runs `f`, then puts the pasteboard back exactly as it was.
pub fn with_saved<T>(f: impl FnOnce() -> T) -> T {
    let saved = save();
    let result = f();
    restore(saved);
    result
}

/// Editors such as VS Code copy the whole line (newline included) when nothing is selected.
pub fn looks_like_selection(text: &str) -> bool {
    !text.is_empty() && !text.ends_with('\n')
}

/// Copies the current selection with Cmd-C and returns it; `None` when nothing is selected.
pub fn copy_selection() -> Option<String> {
    let pb = NSPasteboard::generalPasteboard();
    let before = pb.changeCount();
    inject::command_key(KEYCODE_C);
    let started = Instant::now();
    while started.elapsed() < COPY_TIMEOUT {
        if pb.changeCount() != before {
            return pb.stringForType(unsafe { NSPasteboardTypeString }).map(|s| s.to_string());
        }
        thread::sleep(Duration::from_millis(10));
    }
    None
}

/// Puts `text` on the pasteboard and pastes it with Cmd-V.
pub fn paste(text: &str) {
    let pb = NSPasteboard::generalPasteboard();
    pb.clearContents();
    pb.setString_forType(&NSString::from_str(text), unsafe { NSPasteboardTypeString });
    inject::command_key(KEYCODE_V);
    thread::sleep(PASTE_SETTLE);
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Mutex;

    /// The tests share the one system pasteboard.
    static PASTEBOARD: Mutex<()> = Mutex::new(());

    const CUSTOM: &str = "com.lang-changer.test";

    fn write_two_flavors(text: &str, custom: &[u8]) {
        let pb = NSPasteboard::generalPasteboard();
        pb.clearContents();
        let item = NSPasteboardItem::new();
        item.setString_forType(&NSString::from_str(text), unsafe { NSPasteboardTypeString });
        item.setData_forType(&NSData::with_bytes(custom), &NSString::from_str(CUSTOM));
        let items = NSArray::from_retained_slice(&[ProtocolObject::from_retained(item)]);
        assert!(pb.writeObjects(&items));
    }

    fn read_two_flavors() -> (Option<String>, Option<Vec<u8>>) {
        let pb = NSPasteboard::generalPasteboard();
        let text = pb.stringForType(unsafe { NSPasteboardTypeString }).map(|s| s.to_string());
        let custom = pb
            .pasteboardItems()
            .and_then(|items| items.iter().next())
            .and_then(|item| item.dataForType(&NSString::from_str(CUSTOM)))
            .map(|d| d.to_vec());
        (text, custom)
    }

    #[test]
    fn with_saved_restores_every_flavor() {
        let _guard = PASTEBOARD.lock().unwrap_or_else(|e| e.into_inner());
        let user_clipboard = save(); // leave the developer's clipboard as we found it
        write_two_flavors("original", &[1, 2, 3]);

        with_saved(|| write_two_flavors("temporary", &[9]));

        assert_eq!(read_two_flavors(), (Some("original".to_string()), Some(vec![1, 2, 3])));
        restore(user_clipboard);
    }

    #[test]
    fn whole_line_copies_are_not_selections() {
        assert!(looks_like_selection("ghbdtn"));
        assert!(looks_like_selection("two words"));
        // editors such as VS Code copy the whole line, newline included, when nothing is selected
        assert!(!looks_like_selection("some line\n"));
        assert!(!looks_like_selection(""));
    }

    #[test]
    fn with_saved_returns_the_closure_value() {
        let _guard = PASTEBOARD.lock().unwrap_or_else(|e| e.into_inner());
        let user_clipboard = save();
        assert_eq!(with_saved(|| 42), 42);
        restore(user_clipboard);
    }
}
