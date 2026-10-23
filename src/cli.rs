#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Run,
    InstallAgent,
    UninstallAgent,
    Help,
    Unknown(String),
}

pub const USAGE: &str = "lang-changer: fixes text typed in the wrong RU/EN layout

USAGE:
    lang-changer                  run in the foreground (menu bar item + keyboard tap)
    lang-changer install-agent    start automatically at login (LaunchAgent)
    lang-changer uninstall-agent  remove the LaunchAgent
    lang-changer --help           show this help
";

pub fn parse(args: &[String]) -> Command {
    match args.first().map(String::as_str) {
        None => Command::Run,
        Some("install-agent") => Command::InstallAgent,
        Some("uninstall-agent") => Command::UninstallAgent,
        Some("help" | "--help" | "-h") => Command::Help,
        Some(other) => Command::Unknown(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn no_arguments_runs_the_daemon() {
        assert_eq!(parse(&args(&[])), Command::Run);
    }

    #[test]
    fn agent_subcommands() {
        assert_eq!(parse(&args(&["install-agent"])), Command::InstallAgent);
        assert_eq!(parse(&args(&["uninstall-agent"])), Command::UninstallAgent);
    }

    #[test]
    fn help_flags() {
        assert_eq!(parse(&args(&["--help"])), Command::Help);
        assert_eq!(parse(&args(&["-h"])), Command::Help);
        assert_eq!(parse(&args(&["help"])), Command::Help);
    }

    #[test]
    fn unknown_argument_is_reported() {
        assert_eq!(parse(&args(&["frobnicate"])), Command::Unknown("frobnicate".into()));
    }
}
