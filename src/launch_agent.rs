use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const LABEL: &str = "com.lang-changer";

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// LaunchAgent definition: start at login, restart only after a crash (so menu-bar Quit sticks).
pub fn plist(exe: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <dict>
        <key>SuccessfulExit</key>
        <false/>
    </dict>
</dict>
</plist>
"#,
        exe = xml_escape(exe)
    )
}

pub fn plist_path(home: &Path) -> PathBuf {
    home.join("Library").join("LaunchAgents").join(format!("{LABEL}.plist"))
}

fn gui_domain() -> Result<String, String> {
    let out = Command::new("id").arg("-u").output().map_err(|e| e.to_string())?;
    Ok(format!("gui/{}", String::from_utf8_lossy(&out.stdout).trim()))
}

fn launchctl(args: &[&str]) -> Result<(), String> {
    let status = Command::new("launchctl").args(args).status().map_err(|e| e.to_string())?;
    if status.success() { Ok(()) } else { Err(format!("launchctl {} failed: {status}", args.join(" "))) }
}

pub fn install(home: &Path) -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let path = plist_path(home);
    fs::create_dir_all(path.parent().expect("plist path has a parent")).map_err(|e| e.to_string())?;
    fs::write(&path, plist(&exe.to_string_lossy())).map_err(|e| e.to_string())?;
    let domain = gui_domain()?;
    let _ = launchctl(&["bootout", &format!("{domain}/{LABEL}")]); // replace a previous install
    launchctl(&["bootstrap", &domain, &path.to_string_lossy()])?;
    Ok(path)
}

pub fn uninstall(home: &Path) -> Result<(), String> {
    let domain = gui_domain()?;
    let _ = launchctl(&["bootout", &format!("{domain}/{LABEL}")]);
    match fs::remove_file(plist_path(home)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn plist_runs_the_binary_at_login_and_restarts_only_after_a_crash() {
        let p = plist("/usr/local/bin/lang-changer");
        assert!(p.contains("<key>Label</key>\n    <string>com.lang-changer</string>"));
        assert!(p.contains("<string>/usr/local/bin/lang-changer</string>"));
        assert!(p.contains("<key>RunAtLoad</key>\n    <true/>"));
        assert!(p.contains("<key>SuccessfulExit</key>\n        <false/>"));
    }

    #[test]
    fn plist_escapes_xml_in_paths() {
        let p = plist("/Users/a&b/<x>/lang-changer");
        assert!(p.contains("/Users/a&amp;b/&lt;x&gt;/lang-changer"));
        assert!(!p.contains("a&b"));
    }

    #[test]
    fn plist_lives_in_the_user_launch_agents_dir() {
        assert_eq!(
            plist_path(Path::new("/Users/me")),
            Path::new("/Users/me/Library/LaunchAgents/com.lang-changer.plist")
        );
    }
}
