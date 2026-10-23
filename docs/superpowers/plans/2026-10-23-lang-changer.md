# lang-changer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A macOS background utility in Rust that auto-corrects words typed in the wrong RU/EN layout at word boundaries, with a hotkey for manual convert and undo.

**Architecture:** A `CGEventTap` feeds key events into a `WordBuffer`. At a word boundary a pure `detector` decides Keep/Convert using embedded n-gram models; `layout_map` converts the text; `inject` replaces it by synthetic events and `input_source` switches layout. The hotkey path reuses the same pieces (selection via clipboard, undo via last-fix record).

**Tech Stack:** Rust 1.93, `core-graphics`, `core-foundation`, thin FFI to Carbon Text Input Sources, `serde` + `toml` for config, `tray-icon` (or equivalent) for the menu bar.

**Spec:** `docs/superpowers/specs/2026-10-23-lang-changer-design.md`

## Global Constraints

- macOS only; Rust; few dependencies; no runtime dictionary files (models embedded in the binary).
- Auto-correction enabled in all apps; no default app exclusions.
- Languages: RU ↔ EN only.
- Words shorter than 3 chars are never converted (except entries in the embedded frequent-word list).
- Never convert words containing digits, `_`, `-`, `/`, `.` inside the word, CamelCase, URL/path/email-like tokens, or words in the "leave alone" list.
- Auto path must not touch the clipboard; selection path must restore it.
- Default hotkey `Control+F` (swallowed by the tap, not passed to the app), configurable. Config: `~/.config/lang-changer/config.toml`; leave-alone list in the same directory.
- Needs Accessibility and Input Monitoring permissions; silent in secure-input fields.
- LaunchAgent plist is installed only by an explicit documented command.

## Review Focus

- Password / secure-input field: tap yields no events; the tool must not crash or log typed text.
- Fast typing across a boundary (next key arrives while replacement is being injected): own synthetic events must be ignored, and no characters lost or reordered.
- Identifier-like tokens (`getUserName`, `foo_bar`, `v2.1`, `https://x.y`) typed in Terminal/IDE: never converted.
- Short valid words in either language («да», «ну», `ok`, `to`) and a correctly typed word right after a layout switch: not converted.
- Undo: Backspace or hotkey straight after an auto-fix restores the original text exactly (case included) and the word is not auto-fixed again.

---

### Task 1: Project scaffold and layout map

**Files:**
- Create: `Cargo.toml`, `src/main.rs`, `src/lib.rs`, `src/layout_map.rs`, `.gitignore`
- Test: `src/layout_map.rs` (`#[cfg(test)]`)

**Interfaces:**
- Produces: `pub enum Direction { EnToRu, RuToEn }`; `pub fn convert(text: &str, dir: Direction) -> String`; `pub fn detect_direction(text: &str) -> Direction` (by majority of Latin vs Cyrillic letters).

- [ ] **Step 1:** `git init`, `cargo init --name lang-changer`, split into `lib.rs` (logic) + `main.rs` (binary); add `target/` to `.gitignore`.
- [ ] **Step 2: Write failing tests** in `layout_map.rs`: `converts_en_to_ru` (`"ghbdtn"` → `"привет"`), `converts_ru_to_en` (`"руддщ"` → `"hello"`), `preserves_case` (`"Ghbdtn"` → `"Привет"`), `maps_punctuation_keys` (`"[];',."` → `"хъжэбю"`), `roundtrip_letters` (every mapped letter converts back to itself), `detect_direction_mixed` (more Cyrillic letters → `RuToEn`).
- [ ] **Step 3:** Run `cargo test layout_map` — expect FAIL (not implemented).
- [ ] **Step 4:** Implement the functions with a static table for the standard macOS "Russian" (ЙЦУКЕН) layout against US QWERTY; unmapped chars pass through unchanged.
- [ ] **Step 5:** Run `cargo test layout_map` — expect PASS.
- [ ] **Step 6:** Commit `feat: scaffold project and layout map`.

### Task 2: Detector

**Files:**
- Create: `src/detector.rs`, `data/en_words.txt`, `data/ru_words.txt`
- Test: `src/detector.rs` (`#[cfg(test)]`)

**Interfaces:**
- Consumes: `layout_map::{convert, detect_direction, Direction}`
- Produces: `pub enum Decision { Keep, Convert(Direction) }`; `pub struct Detector`; `Detector::new(threshold: f32) -> Detector` (builds trigram models from the embedded word lists via `include_str!`); `Detector::decide(&self, word: &str, leave_alone: &HashSet<String>) -> Decision`.

- [ ] **Step 1: Write failing tests**: `converts_wrong_layout_ru` (`"ghbdtn"` → `Convert(EnToRu)`), `converts_wrong_layout_en` (`"руддщ"` → `Convert(RuToEn)`), `keeps_correct_words` (`"hello"`, `"привет"` → `Keep`), `keeps_short_words` (`"ok"`, `"да"`, `"to"`), `keeps_identifiers` (`"getUserName"`, `"foo_bar"`, `"v2.1"`, `"https://x.y"`, `"a-b"`), `keeps_leave_alone_words`, `keeps_digits`.
- [ ] **Step 2:** Run `cargo test detector` — expect FAIL.
- [ ] **Step 3:** Implement. Score = mean log-probability of character trigrams under the model for the word's language, compared between typed form and converted form; `Convert` only if converted score minus typed score exceeds `threshold` and typed score is below an implausibility floor. Apply the Global Constraints exclusions before scoring. Word lists: any permissively licensed frequency lists (a few thousand words each); record their source in `data/README.md`.
- [ ] **Step 4:** Run `cargo test detector` — expect PASS; tune threshold defaults until all listed cases pass.
- [ ] **Step 5:** Commit `feat: wrong-layout detector`.

### Task 3: Word buffer

**Files:**
- Create: `src/buffer.rs`
- Test: `src/buffer.rs` (`#[cfg(test)]`)

**Interfaces:**
- Produces: `pub struct WordBuffer`; `push(&mut self, ch: char)`; `pop(&mut self)` (Backspace); `reset(&mut self)`; `take(&mut self) -> String`; `is_empty(&self) -> bool`. Keys are translated to chars by the tap using the *active layout's* output, so the buffer stores what the user actually sees.

- [ ] **Step 1: Write failing tests**: `accumulates_chars`, `backspace_removes_last`, `reset_clears`, `take_returns_and_clears`.
- [ ] **Step 2–4:** Run (FAIL), implement, run (PASS).
- [ ] **Step 5:** Commit `feat: word buffer`.

### Task 4: Config and leave-alone store

**Files:**
- Create: `src/config.rs`
- Test: `src/config.rs` (`#[cfg(test)]`, using a temp dir)

**Interfaces:**
- Produces: `pub struct Config { pub hotkey: Hotkey, pub threshold: f32, pub enabled: bool }` with `Config::load(dir: &Path) -> Config` (defaults if missing/invalid, never panics); `pub struct LeaveAlone`; `LeaveAlone::load(dir: &Path)`, `add(&mut self, word: &str)` (persists immediately), `set(&self) -> &HashSet<String>`.

- [ ] **Step 1: Write failing tests**: `defaults_when_missing` (hotkey `Control+F`), `invalid_toml_falls_back_to_defaults`, `leave_alone_persists_across_loads`.
- [ ] **Step 2–4:** Run (FAIL), implement with `serde`/`toml`, run (PASS).
- [ ] **Step 5:** Commit `feat: config and leave-alone list`.

### Task 5: macOS I/O — input source and text injection

**Files:**
- Create: `src/input_source.rs`, `src/inject.rs`

**Interfaces:**
- Produces: `input_source::switch_to(layout: Direction)` (selects the RU layout for `EnToRu`, the US layout for `RuToEn`, via `TISSelectInputSource`); `input_source::current_is_russian() -> bool`; `inject::replace(backspaces: usize, text: &str)` — posts Backspace events then the text via `CGEventKeyboardSetUnicodeString`, every event tagged with a private `EventSourceUserData` marker; `inject::is_own_event(&CGEvent) -> bool`.

- [ ] **Step 1:** Implement both modules (FFI declarations for Carbon `TISCopyCurrentKeyboardInputSource`, `TISCreateInputSourceList`, `TISSelectInputSource`).
- [ ] **Step 2: Manual check** via a throwaway `examples/io_smoke.rs`: run it, focus Notes, expect «привет» typed after 3 seconds and layout switched to Russian.
- [ ] **Step 3:** Commit `feat: input source switching and text injection`.

### Task 6: Event tap and auto-fix loop

**Files:**
- Create: `src/tap.rs`; Modify: `src/main.rs`

**Interfaces:**
- Consumes: `WordBuffer`, `Detector`, `LeaveAlone`, `Config`, `inject`, `input_source`.
- Produces: `tap::run(config, detector, leave_alone)` — installs a listen/modify `CGEventTap` on key-down, runs the run loop. Keeps `last_fix: Option<{original: String, boundary: char}>`.

- [ ] **Step 1:** Implement: ignore events where `inject::is_own_event`; translate key-down to a char via `CGEventKeyboardGetUnicodeString`; push to the buffer; reset on Cmd/Ctrl-modified keys (except the hotkey, which is consumed), navigation keys, and on mouse-down and app-activation notifications; on boundary char call `detector.decide`; on `Convert`, `inject::replace(word_len + 1, converted + boundary)`, `input_source::switch_to`, record `last_fix`.
- [ ] **Step 2:** Undo: the hotkey or Backspace immediately after a fix restores `original + boundary` via `inject::replace`, adds the word to `LeaveAlone`, clears `last_fix`. Any other key clears `last_fix`.
- [ ] **Step 3:** Request Accessibility/Input Monitoring on startup (`AXIsProcessTrustedWithOptions`) with a clear console message if denied.
- [ ] **Step 4: Manual check**: `cargo run --release`, in Notes type `ghbdtn ` → «привет »; type `hello ` → unchanged; `getUserName ` unchanged; undo by Backspace restores `ghbdtn `.
- [ ] **Step 5:** Commit `feat: event tap with auto-fix and undo`.

### Task 7: Manual hotkey and selection conversion

**Files:**
- Modify: `src/tap.rs`; Create: `src/clipboard.rs`

**Interfaces:**
- Produces: `clipboard::with_saved<T>(f: impl FnOnce() -> T) -> T` (saves and restores all pasteboard items); `clipboard::copy_selection() -> Option<String>` (posts Cmd-C, reads pasteboard); `clipboard::paste(text: &str)`.

- [ ] **Step 1:** Hotkey with no recent fix: if a selection exists (`copy_selection` returns text different from the old pasteboard contents), convert with `detect_direction` and paste it inside `with_saved`; otherwise convert the buffer's last word with `inject::replace`.
- [ ] **Step 2: Manual check**: select `ghbdtn` in Notes, press hotkey → «привет»; clipboard content from before is unchanged.
- [ ] **Step 3:** Commit `feat: manual hotkey and selection conversion`.

### Task 8: Menu bar, LaunchAgent, docs

**Files:**
- Create: `src/menubar.rs`, `README.md`, `scripts/com.lang-changer.plist`; Modify: `src/main.rs`, `src/tap.rs`

- [ ] **Step 1:** Menu bar item with "Enabled" toggle (writes `Config.enabled`, tap checks it) and "Quit".
- [ ] **Step 2:** `lang-changer install-agent` / `uninstall-agent` subcommands that write/remove `~/Library/LaunchAgents/com.lang-changer.plist` and `launchctl load/unload` it.
- [ ] **Step 3:** README: build, permissions, config keys, manual test checklist (Terminal, Safari, Notes, VS Code, password field).
- [ ] **Step 4: Manual checklist** from the README passes; `cargo test` and `cargo clippy -- -D warnings` clean.
- [ ] **Step 5:** Commit `feat: menu bar, launch agent, docs`.

---

## Self-review

- Spec coverage: auto-fix (T2, T6), hotkey + selection (T7), undo + leave-alone (T4, T6), layout switch (T5), case/punctuation (T1), menu bar (T8), permissions/secure input (T6), config (T4), LaunchAgent (T8), tests (T1–T4 unit, T5–T8 manual).
- Review Focus coverage: secure input (T6 step 3/4), own-event loop (T5 `is_own_event`, T6 step 1), identifiers (T2 `keeps_identifiers`), short words (T2 `keeps_short_words`), undo (T6 step 2/4).
- Known weak spot: detector quality depends on the word lists; threshold tuning in T2 step 4 is the risk.
