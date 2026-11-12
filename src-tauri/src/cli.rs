//! Command-line options: `--hidden` and `--apply-profile <name>`.

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Cli {
    /// Start in the tray without showing the window.
    pub hidden: bool,
    pub apply_profile: Option<String>,
}

impl Cli {
    /// Parses arguments (without the program name). Unknown arguments are ignored.
    pub fn parse<I: IntoIterator<Item = String>>(args: I) -> Self {
        let mut cli = Cli::default();
        let mut it = args.into_iter().peekable();
        while let Some(a) = it.next() {
            if a == "--hidden" {
                cli.hidden = true;
            } else if a == "--apply-profile" {
                cli.apply_profile = it.next_if(|n| !n.starts_with("--"));
            } else if let Some(n) = a.strip_prefix("--apply-profile=") {
                cli.apply_profile = Some(n.to_string());
            }
        }
        cli
    }

    /// Whether the main window starts hidden (applying a profile never needs a window).
    pub fn windowless(&self, start_hidden_setting: bool) -> bool {
        self.hidden || self.apply_profile.is_some() || start_hidden_setting
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(a: &[&str]) -> Cli {
        Cli::parse(a.iter().map(|s| s.to_string()))
    }

    #[test]
    fn parses_flags() {
        assert_eq!(p(&[]), Cli::default());
        assert!(p(&["--hidden"]).hidden);
        assert_eq!(p(&["--apply-profile", "Night mode"]).apply_profile.as_deref(), Some("Night mode"));
        assert_eq!(p(&["--apply-profile=Gaming", "--hidden"]), Cli { hidden: true, apply_profile: Some("Gaming".into()) });
    }

    #[test]
    fn missing_value_and_unknown_args() {
        assert_eq!(p(&["--apply-profile"]).apply_profile, None);
        assert_eq!(p(&["--apply-profile", "--hidden"]), Cli { hidden: true, apply_profile: None });
        assert_eq!(p(&["--whatever", "x"]), Cli::default());
    }

    #[test]
    fn windowless_rules() {
        assert!(!Cli::default().windowless(false));
        assert!(Cli::default().windowless(true));
        assert!(p(&["--apply-profile", "x"]).windowless(false));
    }
}
