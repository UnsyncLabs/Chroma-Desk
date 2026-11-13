//! Command-line options: `--hidden`, `--apply-profile <name>` and the `trigger` subcommand.

use crate::triggers::{DurationSpec, TriggerSpec, parse_color, parse_duration};

#[derive(Debug, Default, PartialEq)]
pub struct Cli {
    /// Start in the tray without showing the window.
    pub hidden: bool,
    pub apply_profile: Option<String>,
    /// `chroma-desk trigger ...`: talk to the running instance and exit.
    pub trigger: Option<Result<TriggerCmd, String>>,
}

#[derive(Debug, PartialEq)]
pub enum TriggerCmd {
    Fire(TriggerSpec),
    Stop,
}

pub const TRIGGER_USAGE: &str = "usage: chroma-desk trigger <animation> [--device <name>] [--keys W,A,S,D] [--duration 3s] [--color #ff0000] [--repeat N] [--priority N]\n       chroma-desk trigger --stop";

/// Parses the arguments after `trigger`.
pub fn parse_trigger<I: IntoIterator<Item = String>>(args: I) -> Result<TriggerCmd, String> {
    let mut spec = TriggerSpec::default();
    let mut stop = false;
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        let (flag, inline) = match a.split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f.to_string(), Some(v.to_string())),
            _ => (a.clone(), None),
        };
        let mut value = |name: &str| inline.clone().or_else(|| it.next()).ok_or_else(|| format!("{name} needs a value"));
        match flag.as_str() {
            "--stop" => stop = true,
            "--device" | "-d" => spec.device = Some(value("--device")?),
            "--keys" | "-k" => spec.keys.extend(value("--keys")?.split(',').map(|k| k.trim().to_string()).filter(|k| !k.is_empty())),
            "--duration" | "-t" => {
                let v = value("--duration")?;
                parse_duration(&v).ok_or_else(|| format!("bad duration {v:?} (try 3s or 500ms)"))?;
                spec.duration = Some(DurationSpec::Text(v));
            }
            "--color" | "-c" => {
                let v = value("--color")?;
                parse_color(&v).ok_or_else(|| format!("bad colour {v:?} (try #ff0000)"))?;
                spec.color = Some(v);
            }
            "--repeat" | "-r" => {
                let v = value("--repeat")?;
                spec.repeat = Some(v.parse().map_err(|_| format!("bad repeat count {v:?}"))?);
            }
            "--priority" | "-p" => {
                let v = value("--priority")?;
                spec.priority = v.parse().map_err(|_| format!("bad priority {v:?}"))?;
            }
            f if f.starts_with('-') && f.len() > 1 => return Err(format!("unknown option {f}")),
            _ if spec.animation.is_empty() => spec.animation = a,
            _ => return Err(format!("unexpected argument {a:?}")),
        }
    }
    if stop {
        return Ok(TriggerCmd::Stop);
    }
    if spec.animation.trim().is_empty() {
        return Err("missing animation name".into());
    }
    spec.validate().map_err(|e| format!("{}: {}", e.code, e.detail))?;
    Ok(TriggerCmd::Fire(spec))
}

/// Sends a trigger command to the running instance; returns the process exit code.
pub fn run_trigger(cmd: &Result<TriggerCmd, String>) -> i32 {
    let cmd = match cmd {
        Ok(c) => c,
        Err(e) => {
            eprintln!("chroma-desk: {e}\n{TRIGGER_USAGE}");
            return 2;
        }
    };
    let line = match cmd {
        TriggerCmd::Stop => serde_json::json!({ "cmd": "stop" }),
        TriggerCmd::Fire(spec) => {
            let mut v = serde_json::to_value(spec).unwrap_or_default();
            v["cmd"] = "trigger".into();
            v
        }
    };
    let Some(path) = crate::socket::socket_path() else {
        eprintln!("chroma-desk: XDG_RUNTIME_DIR is not set; cannot find the running instance");
        return 1;
    };
    match crate::socket::send(&path, &line.to_string()) {
        Ok(reply) => {
            println!("{reply}");
            let ok = serde_json::from_str::<serde_json::Value>(&reply).is_ok_and(|v| v["ok"] == true);
            i32::from(!ok)
        }
        Err(e) => {
            eprintln!("chroma-desk: is the app running? could not reach {} ({e})", path.display());
            1
        }
    }
}

impl Cli {
    /// Parses arguments (without the program name). Unknown arguments are ignored.
    pub fn parse<I: IntoIterator<Item = String>>(args: I) -> Self {
        let mut cli = Cli::default();
        let mut it = args.into_iter().peekable();
        if it.peek().is_some_and(|a| a == "trigger") {
            it.next();
            cli.trigger = Some(parse_trigger(it));
            return cli;
        }
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
        assert_eq!(p(&["--apply-profile=Gaming", "--hidden"]), Cli { hidden: true, apply_profile: Some("Gaming".into()), trigger: None });
    }

    #[test]
    fn missing_value_and_unknown_args() {
        assert_eq!(p(&["--apply-profile"]).apply_profile, None);
        assert_eq!(p(&["--apply-profile", "--hidden"]), Cli { hidden: true, apply_profile: None, trigger: None });
        assert_eq!(p(&["--whatever", "x"]), Cli::default());
    }

    #[test]
    fn windowless_rules() {
        assert!(!Cli::default().windowless(false));
        assert!(Cli::default().windowless(true));
        assert!(p(&["--apply-profile", "x"]).windowless(false));
    }

    fn trig(a: &[&str]) -> Result<TriggerCmd, String> {
        parse_trigger(a.iter().map(|s| s.to_string()))
    }

    #[test]
    fn trigger_full_command() {
        let Ok(TriggerCmd::Fire(s)) = trig(&["Alerta", "--device", "razer", "--keys", "W,A, S ,D", "--duration", "2s", "--color", "#ff0000", "--repeat", "3", "--priority", "5"]) else {
            panic!("parse failed")
        };
        assert_eq!(s.animation, "Alerta");
        assert_eq!(s.device.as_deref(), Some("razer"));
        assert_eq!(s.keys, vec!["W", "A", "S", "D"]);
        assert_eq!(s.duration, Some(DurationSpec::Text("2s".into())));
        assert_eq!((s.color.as_deref(), s.repeat, s.priority), (Some("#ff0000"), Some(3), 5));
    }

    #[test]
    fn trigger_inline_values_and_order() {
        let Ok(TriggerCmd::Fire(s)) = trig(&["--keys=Q,E", "--duration=500ms", "Pulse"]) else { panic!() };
        assert_eq!((s.animation.as_str(), s.keys.len()), ("Pulse", 2));
        assert_eq!(trig(&["--stop"]), Ok(TriggerCmd::Stop));
        let Ok(TriggerCmd::Fire(s)) = trig(&["Multi word name"]) else { panic!() };
        assert_eq!(s.animation, "Multi word name");
    }

    #[test]
    fn trigger_errors() {
        for bad in [
            &[][..],
            &["--keys", "W"][..],
            &["A", "B"][..],
            &["A", "--duration", "soon"][..],
            &["A", "--color", "red"][..],
            &["A", "--repeat", "x"][..],
            &["A", "--repeat", "0"][..],
            &["A", "--bogus"][..],
            &["A", "--keys"][..],
        ] {
            assert!(trig(bad).is_err(), "{bad:?} should fail");
        }
    }

    #[test]
    fn trigger_is_a_subcommand_of_the_binary() {
        assert!(matches!(p(&["trigger", "X"]).trigger, Some(Ok(TriggerCmd::Fire(_)))));
        assert!(matches!(p(&["trigger"]).trigger, Some(Err(_))));
        assert!(p(&["--hidden"]).trigger.is_none());
    }
}
