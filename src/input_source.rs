use crate::layout_map::Direction;
use core_foundation::array::{CFArrayGetCount, CFArrayGetValueAtIndex, CFArrayRef};
use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
use core_foundation::string::{CFString, CFStringRef};
use std::ffi::c_void;
use std::ptr;

type TISInputSourceRef = *const c_void;

#[link(name = "Carbon", kind = "framework")]
extern "C" {
    static kTISPropertyInputSourceID: CFStringRef;
    fn TISCopyCurrentKeyboardInputSource() -> TISInputSourceRef;
    fn TISCreateInputSourceList(properties: *const c_void, include_all_installed: u8) -> CFArrayRef;
    fn TISGetInputSourceProperty(source: TISInputSourceRef, key: CFStringRef) -> *const c_void;
    fn TISSelectInputSource(source: TISInputSourceRef) -> i32;
}

fn source_id(source: TISInputSourceRef) -> Option<String> {
    // SAFETY: `source` is a live TISInputSourceRef; the property is a CFString owned by it.
    unsafe {
        let p = TISGetInputSourceProperty(source, kTISPropertyInputSourceID);
        if p.is_null() {
            return None;
        }
        Some(CFString::wrap_under_get_rule(p as CFStringRef).to_string())
    }
}

pub fn current_id() -> Option<String> {
    // SAFETY: the copied source follows the create rule, so it is released here.
    unsafe {
        let cur = TISCopyCurrentKeyboardInputSource();
        if cur.is_null() {
            return None;
        }
        let id = source_id(cur);
        CFRelease(cur as CFTypeRef);
        id
    }
}

pub fn current_is_russian() -> bool {
    current_id().is_some_and(|id| id.contains("Russian"))
}

fn wants(id: &str, target: Direction) -> bool {
    match target {
        Direction::EnToRu => id.starts_with("com.apple.keylayout.") && id.contains("Russian"),
        Direction::RuToEn => id == "com.apple.keylayout.US" || id == "com.apple.keylayout.ABC",
    }
}

/// Selects the layout the conversion produced text for: Russian for `EnToRu`, US/ABC for `RuToEn`.
pub fn switch_to(direction: Direction) {
    // SAFETY: the list follows the create rule and is released after use; elements are borrowed.
    unsafe {
        let list = TISCreateInputSourceList(ptr::null(), 0);
        if list.is_null() {
            return;
        }
        for i in 0..CFArrayGetCount(list) {
            let src = CFArrayGetValueAtIndex(list, i);
            if source_id(src).is_some_and(|id| wants(&id, direction)) {
                TISSelectInputSource(src);
                break;
            }
        }
        CFRelease(list as CFTypeRef);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_matching_layout_ids() {
        assert!(wants("com.apple.keylayout.Russian", Direction::EnToRu));
        assert!(wants("com.apple.keylayout.RussianWin", Direction::EnToRu));
        assert!(!wants("com.apple.keylayout.US", Direction::EnToRu));
        assert!(wants("com.apple.keylayout.US", Direction::RuToEn));
        assert!(wants("com.apple.keylayout.ABC", Direction::RuToEn));
        assert!(!wants("com.apple.keylayout.Russian", Direction::RuToEn));
    }

    #[test]
    fn reads_current_layout_id() {
        // Needs a logged-in GUI session; prints the id for the manual record.
        let id = current_id();
        println!("current input source: {id:?}");
        assert!(id.is_some());
    }
}
