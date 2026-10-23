# lang-changer

A small macOS background tool that fixes text typed in the wrong keyboard layout (RU ↔ EN), like PuntoSwitcher.

- Type `ghbdtn` and press space → it becomes `привет ` and the layout switches to Russian.
- `Control+F` converts the selection, or the last word if nothing is selected.
- `Control+F` (or Backspace) right after an auto-fix undoes it and remembers the word.

## Build and run

```sh
cargo build --release
./target/release/lang-changer
```

A `Ru⇄En` item appears in the menu bar with an "Auto-fix enabled" toggle and Quit.

### Permissions

macOS asks for two permissions on first start. Grant both to the binary (or to the terminal that runs it) in
System Settings → Privacy & Security, then start it again:

- Accessibility
- Input Monitoring

In password fields macOS disables keyboard monitoring, so the tool stays silent there.

### Start at login

```sh
./target/release/lang-changer install-agent     # writes ~/Library/LaunchAgents/com.lang-changer.plist
./target/release/lang-changer uninstall-agent
```

The permissions must be granted to the binary path that the agent runs. Restarts only after a crash, so Quit sticks.

## Configuration

`~/.config/lang-changer/config.toml` (all keys optional):

```toml
hotkey = "ctrl+f"   # modifiers ctrl/alt/cmd/shift + one key (letter, digit or "space")
threshold = 1.0     # higher = more cautious auto-fix
enabled = true      # auto-fix on at start
```

`Control+F` is swallowed while the tool runs, so it no longer moves the cursor forward in text fields.
Words you undid are stored in `~/.config/lang-changer/leave_alone.txt` (one per line, delete a line to forget it).

## What is never auto-fixed

Words shorter than 3 characters (unless they are frequent words), words with digits, `_`, `-`, `/`, `@`, `:`,
a `.` inside, CamelCase, and words the detector recognises as valid in the layout they were typed in.
Enter and Tab never trigger an auto-fix; use the hotkey for the last word instead.

## Manual test checklist

Run in Terminal, Safari, Notes and VS Code:

- [ ] `ghbdtn ` → `привет `, layout is now Russian
- [ ] `руддщ ` on the English layout → `hello `
- [ ] `hello `, `да `, `getUserName `, `foo_bar `, `v2.1 ` stay as typed
- [ ] `ghbdtn! ` → `привет! `; `ghbdtn, ` → `привет, `
- [ ] Backspace right after an auto-fix restores `ghbdtn `; typing it again is left alone
- [ ] Select `ghbdtn`, press `Control+F` → `привет`; the previous clipboard content is unchanged
- [ ] With nothing selected, `nginx` then `Control+F` converts the word
- [ ] Menu bar toggle off → no auto-fix, hotkey still works
- [ ] Password field: nothing happens, no crash

## Word lists

`data/` holds the frequency lists used by the detector; see `data/README.md` for their source and license.
