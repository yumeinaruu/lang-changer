use crate::clipboard;
use crate::config::Hotkey;
use crate::engine::{Engine, KeyInput};
use crate::inject;
use crate::injector::Injector;
use crate::input_source;
use crate::keys::{classify, Mods};
use crate::layout_map::{convert, detect_direction};
use core_foundation::base::TCFType;
use foreign_types::ForeignType;
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::runloop::{kCFRunLoopCommonModes, CFRunLoop};
use core_foundation::string::{CFString, CFStringRef};
use core_graphics::event::{
    CGEvent, CGEventFlags, CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
    CGEventType, CallbackResult, EventField,
};
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSWorkspace};
use std::ffi::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Lets the key that ended the word reach the app before our backspaces do.
const REPLACE_DELAY: Duration = Duration::from_millis(8);

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    static kAXTrustedCheckOptionPrompt: CFStringRef;
    fn AXIsProcessTrustedWithOptions(options: *const c_void) -> bool;
}

extern "C" {
    static _dispatch_main_q: c_void;
    fn dispatch_async_f(queue: *const c_void, context: *mut c_void, work: extern "C" fn(*mut c_void));
}

/// Runs `f` on the main thread (the run loop services the main dispatch queue).
fn run_on_main(f: impl FnOnce() + Send + 'static) {
    extern "C" fn trampoline(context: *mut c_void) {
        // SAFETY: `context` is the Box<Box<dyn FnOnce()>> leaked below, consumed exactly once.
        let f = unsafe { Box::from_raw(context as *mut Box<dyn FnOnce() + Send>) };
        f();
    }
    let boxed: Box<Box<dyn FnOnce() + Send>> = Box::new(Box::new(f));
    // SAFETY: `_dispatch_main_q` is libdispatch's main queue; ownership of `boxed` moves to the trampoline.
    unsafe { dispatch_async_f(&raw const _dispatch_main_q, Box::into_raw(boxed) as *mut c_void, trampoline) };
}

/// Hotkey with nothing buffered: convert the current selection through the pasteboard.
fn convert_selection() {
    std::thread::spawn(|| {
        let direction = clipboard::with_saved(|| {
            let text = clipboard::copy_selection().filter(|t| clipboard::looks_like_selection(t))?;
            let direction = detect_direction(&text);
            clipboard::paste(&convert(&text, direction));
            Some(direction)
        });
        if let Some(direction) = direction {
            run_on_main(move || input_source::switch_to(direction));
        }
    });
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventKeyboardGetUnicodeString(event: *const c_void, max_len: usize, actual: *mut usize, buf: *mut u16);
    fn CGEventTapEnable(tap: *const c_void, enable: bool);
    fn CGPreflightListenEventAccess() -> bool;
    fn CGRequestListenEventAccess() -> bool;
}

/// Asks macOS for Accessibility and Input Monitoring; returns false if either is still missing.
pub fn request_permissions() -> bool {
    // SAFETY: plain system calls; the dictionary is a valid CFDictionary for the call's duration.
    unsafe {
        let key = CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt);
        let options = CFDictionary::from_CFType_pairs(&[(key.as_CFType(), CFBoolean::true_value().as_CFType())]);
        let accessibility = AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef() as *const c_void);
        let input = CGPreflightListenEventAccess() || CGRequestListenEventAccess();
        accessibility && input
    }
}

fn event_text(event: &CGEvent) -> String {
    let mut buf = [0u16; 8];
    let mut len = 0usize;
    // SAFETY: `event` is a live CGEvent and `buf` holds `max_len` UTF-16 units.
    unsafe {
        CGEventKeyboardGetUnicodeString(event.as_ptr() as *const c_void, buf.len(), &mut len, buf.as_mut_ptr());
    }
    String::from_utf16_lossy(&buf[..len.min(buf.len())])
}

struct State {
    engine: Arc<Mutex<Engine>>,
    hotkey: Hotkey,
    injector: Injector,
    last_pid: i32,
}

fn frontmost_pid() -> i32 {
    NSWorkspace::sharedWorkspace().frontmostApplication().map_or(0, |app| app.processIdentifier())
}

/// Installs the tap and blocks on the run loop.
pub fn run(engine: Arc<Mutex<Engine>>, hotkey: Hotkey) -> Result<(), String> {
    let injector = Injector::new(REPLACE_DELAY, inject::replace);
    let state = Mutex::new(State { engine, hotkey, injector, last_pid: 0 });
    let port = Arc::new(AtomicUsize::new(0));
    let port_in_callback = port.clone();

    let tap = CGEventTap::new(
        CGEventTapLocation::HID,
        CGEventTapPlacement::HeadInsertEventTap,
        CGEventTapOptions::Default,
        vec![
            CGEventType::KeyDown,
            CGEventType::LeftMouseDown,
            CGEventType::RightMouseDown,
            CGEventType::OtherMouseDown,
        ],
        move |_proxy, event_type, event| handle(&state, &port_in_callback, event_type, event),
    )
    .map_err(|_| "cannot create the event tap: grant Accessibility and Input Monitoring in System Settings → Privacy & Security".to_string())?;

    port.store(tap.mach_port().as_concrete_TypeRef() as usize, Ordering::SeqCst);
    let source = tap.mach_port().create_runloop_source(0).map_err(|_| "cannot create run loop source".to_string())?;
    // SAFETY: kCFRunLoopCommonModes is a valid static CFString.
    CFRunLoop::get_current().add_source(&source, unsafe { kCFRunLoopCommonModes });
    tap.enable();
    // AppKit's run loop also services the tap source and is what delivers menu bar clicks.
    let mtm = MainThreadMarker::new().ok_or("the event tap must run on the main thread")?;
    NSApplication::sharedApplication(mtm).run();
    Ok(())
}

fn handle(state: &Mutex<State>, port: &AtomicUsize, event_type: CGEventType, event: &CGEvent) -> CallbackResult {
    let mut st = state.lock().unwrap();
    let input = match event_type {
        CGEventType::TapDisabledByTimeout | CGEventType::TapDisabledByUserInput => {
            let p = port.load(Ordering::SeqCst);
            if p != 0 {
                // SAFETY: the port was stored from the live tap, which outlives the run loop.
                unsafe { CGEventTapEnable(p as *const c_void, true) };
            }
            return CallbackResult::Keep;
        }
        CGEventType::KeyDown => {
            if inject::is_own_event(event) {
                return CallbackResult::Keep;
            }
            let pid = frontmost_pid();
            if pid != st.last_pid {
                st.last_pid = pid;
                st.engine.lock().unwrap().on_key(KeyInput::Reset);
            }
            let flags = event.get_flags();
            let mods = Mods {
                ctrl: flags.contains(CGEventFlags::CGEventFlagControl),
                alt: flags.contains(CGEventFlags::CGEventFlagAlternate),
                cmd: flags.contains(CGEventFlags::CGEventFlagCommand),
                shift: flags.contains(CGEventFlags::CGEventFlagShift),
            };
            let keycode = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u16;
            classify(keycode, mods, &event_text(event), &st.hotkey)
        }
        _ => KeyInput::Reset, // mouse click
    };

    let out = st.engine.lock().unwrap().on_key(input);
    let nothing_to_convert = out.replace.is_none();
    if let Some(replace) = out.replace {
        // TIS must be called on the main thread, which this callback runs on.
        input_source::switch_to(replace.switch_to);
        st.injector.submit(replace.backspaces, replace.text);
    }
    if input == KeyInput::Hotkey && nothing_to_convert {
        convert_selection();
    }
    if out.swallow { CallbackResult::Drop } else { CallbackResult::Keep }
}

