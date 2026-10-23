# lang-changer — design

Date: 2026-10-23

## Intent

A small, fast macOS background utility in Rust that fixes text typed in the wrong keyboard layout (RU ↔ EN), like PuntoSwitcher. The user types `ghbdtn`, hits space, and gets «привет»; the layout switches so typing continues. A hotkey does the same manually and also undoes a wrong auto-fix.

Why: the user wants PuntoSwitcher behaviour on macOS with a light, native, dependency-poor tool.

## Goals

- Automatic correction at word boundary (space, Enter, Tab, punctuation), enabled in all apps.
- Manual hotkey: convert selection, or the last typed word if nothing is selected.
- Same hotkey right after an auto-fix reverts it and remembers the word as "leave alone".
- Switch the active input source to the target layout after a fix.
- Preserve case; map RU punctuation keys (х ъ ж э б ю) correctly.
- Low overhead: no perceptible typing lag, binary of a few MB, no runtime dictionary files.

## Non-goals

- Languages other than RU/EN.
- Settings GUI, App Store distribution.
- Cloud or learning-from-history models.

## Anti-goals (failure even if it "works")

- Corrupting text the user typed correctly (false positives).
- Clobbering the clipboard without restoring it.
- Typing lag.

## Behaviour

### Word buffer
Event tap on key-down keeps a buffer of the keycodes typed in the current word. The buffer is reset on: app focus change, mouse click, arrow/navigation keys, Cmd/Ctrl-modified keys (other than the hotkey), and after each boundary.

### Detector (pure function: word → Keep | Convert)
- Compute plausibility of the word as typed vs. converted to the other layout, using embedded character bigram/trigram frequency models for RU and EN plus a small embedded list of frequent words.
- Convert only if the converted form is clearly better and the typed form is implausible (threshold in config).
- Never convert: words shorter than 3 chars (except entries of the frequent-word list), words containing digits, `_`, `-`, `/`, `.` inside the word, CamelCase, URL/path/email-like tokens, words in the user's "leave alone" list.

### Replacement
Backspace × word length (+ boundary char), then type the converted word and the boundary char via `CGEventKeyboardSetUnicodeString`. Clipboard is not touched for the auto path. Selection path (hotkey with selection): save clipboard, Cmd-C, convert, paste, restore clipboard.

### Undo
Hotkey (or Backspace) immediately after an auto-fix restores the original text and adds the word to the "leave alone" list (persisted).

### Hotkey
Default `Control+F` (user's choice, configurable). The tap must swallow this key combo so the app doesn't also receive it; this shadows the macOS Emacs-style "cursor forward" binding.

### Menu bar
Status item with: enabled/paused toggle, quit. Minimal.

### Permissions
Accessibility and Input Monitoring; prompt on first launch with a clear message. macOS secure input (password fields) disables the tap — the tool stays silent there.

## Constraints

- Rust (user's choice), macOS only. Prefer `core-graphics` / `core-foundation` and thin FFI to Text Input Sources over heavy frameworks.
- Config at `~/.config/lang-changer/config.toml`; "leave alone" list in the same directory.
- Run at login via a LaunchAgent plist (installed by a documented command, not automatically).

## Decisions left to the builder

- Exact n-gram model format and how it is generated/embedded; threshold defaults.
- Menu bar crate choice.
- Module layout. Suggested split: `layout_map` (tables), `detector` (pure), `buffer`, `tap` (CGEventTap), `inject` (replacement), `input_source` (TIS), `config`, `main`.

## Testing

- Unit tests: layout mapping round-trips, detector on a corpus of real examples (typed-wrong, typed-right, short words, mixed, identifiers), buffer reset rules.
- Manual checklist in Terminal, Safari, Notes, VS Code: auto-fix, undo, selection convert, password field silence.

## Builder-check notes (my calls, flagged for review)

- Hotkey `Control+F` was chosen by the user; it overrides the system "cursor forward" shortcut while the tool runs. Configurable.
- Terminal/IDE are NOT excluded by default (user asked for "everywhere"); identifier-like tokens are protected by the detector instead.
- Backspace-as-undo is my addition.
