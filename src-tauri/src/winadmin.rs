//! Windows: PawnIO (the driver OpenRGB needs for SMBus devices: RAM, motherboard) and
//! starting chroma-desk elevated at logon through a scheduled task (the autostart Run key
//! cannot elevate). OpenRGB started by an elevated chroma-desk is elevated too.
//! Every function is a no-op / `false` elsewhere.

#[cfg(windows)]
const PAWNIO_URL: &str = "https://github.com/namazso/PawnIO.Setup/releases/download/2.2.0/PawnIO_setup.exe";
#[cfg(windows)]
const PAWNIO_SHA256: &str = "1F519A22E47187F70A1379A48CA604981C4FCF694F4E65B734AAA74A9FBA3032";
#[cfg(windows)]
const TASK: &str = "chroma-desk (admin)";

#[cfg(windows)]
fn powershell(script: &str) -> Result<String, String> {
    let out = crate::fsutil::quiet_command("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script])
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

/// Runs `script` in an elevated PowerShell (UAC prompt) and waits for it.
#[cfg(windows)]
fn powershell_elevated(script: &str) -> Result<(), String> {
    let utf16: Vec<u8> = script.encode_utf16().flat_map(|c| c.to_le_bytes()).collect();
    let encoded = base64(&utf16);
    powershell(&format!(
        "$p = Start-Process powershell.exe -Verb RunAs -Wait -PassThru -WindowStyle Hidden -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-EncodedCommand','{encoded}'; exit $p.ExitCode"
    ))
    .map(|_| ())
    .map_err(|e| if e.to_lowercase().contains("cancel") { "cancelled (UAC)".into() } else { e })
}

#[cfg(windows)]
fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::new();
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            s.push(if i <= c.len() { T[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
        }
    }
    s
}

/// The PawnIO driver service exists.
pub fn pawnio_installed() -> bool {
    #[cfg(windows)]
    {
        crate::fsutil::quiet_command("sc.exe").args(["query", "PawnIO"]).output().is_ok_and(|o| o.status.success())
    }
    #[cfg(not(windows))]
    false
}

/// Downloads the pinned PawnIO installer, checks it and installs it silently (UAC prompt).
pub fn install_pawnio() -> Result<(), String> {
    #[cfg(windows)]
    {
        let t = std::env::temp_dir().join(format!("pawnio-{}.exe", std::process::id()));
        let tp = t.display().to_string().replace('\'', "''");
        powershell(&format!(
            "$ErrorActionPreference='Stop'; $ProgressPreference='SilentlyContinue'; [Net.ServicePointManager]::SecurityProtocol='Tls12'
Invoke-WebRequest -UseBasicParsing -Uri '{PAWNIO_URL}' -OutFile '{tp}'
if ((Get-FileHash -Algorithm SHA256 '{tp}').Hash -ne '{PAWNIO_SHA256}') {{ Remove-Item '{tp}'; throw 'checksum mismatch' }}"
        ))?;
        let r = powershell_elevated(&format!("$p = Start-Process '{tp}' -ArgumentList '-install','-silent' -Wait -PassThru; exit $p.ExitCode"));
        let _ = std::fs::remove_file(&t);
        r?;
        if pawnio_installed() { Ok(()) } else { Err("PawnIO did not install".into()) }
    }
    #[cfg(not(windows))]
    Err("only available on Windows".into())
}

/// This process runs elevated.
pub fn is_elevated() -> bool {
    #[cfg(windows)]
    {
        crate::fsutil::quiet_command("net.exe").arg("session").output().is_ok_and(|o| o.status.success())
    }
    #[cfg(not(windows))]
    false
}

/// The elevated logon task exists.
pub fn admin_autostart() -> bool {
    #[cfg(windows)]
    {
        crate::fsutil::quiet_command("schtasks.exe").args(["/Query", "/TN", TASK]).output().is_ok_and(|o| o.status.success())
    }
    #[cfg(not(windows))]
    false
}

/// Creates or removes the logon task that starts chroma-desk `--hidden` with highest privileges.
pub fn set_admin_autostart(enable: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        let script = if enable {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?.display().to_string().replace('\'', "''");
            let user = format!("{}\\{}", std::env::var("USERDOMAIN").unwrap_or_default(), std::env::var("USERNAME").unwrap_or_default()).replace('\'', "''");
            format!(
                "$a = New-ScheduledTaskAction -Execute '{exe}' -Argument '--hidden'
$t = New-ScheduledTaskTrigger -AtLogOn -User '{user}'
$p = New-ScheduledTaskPrincipal -UserId '{user}' -LogonType Interactive -RunLevel Highest
$s = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit 0
Register-ScheduledTask -TaskName '{TASK}' -Action $a -Trigger $t -Principal $p -Settings $s -Force | Out-Null"
            )
        } else {
            format!("Unregister-ScheduledTask -TaskName '{TASK}' -Confirm:$false")
        };
        powershell_elevated(&script)?;
        if admin_autostart() == enable { Ok(()) } else { Err("the scheduled task was not changed".into()) }
    }
    #[cfg(not(windows))]
    {
        let _ = enable;
        Err("only available on Windows".into())
    }
}

/// Starts the elevated task (no UAC prompt: the task already has the rights) a moment after
/// this instance exits, so single-instance does not hand the launch back to us.
pub fn relaunch_elevated() -> Result<(), String> {
    #[cfg(windows)]
    {
        crate::fsutil::quiet_command("cmd.exe")
            .args(["/C", &format!("timeout /T 2 /NOBREAK >NUL & schtasks /Run /TN \"{TASK}\"")])
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    #[cfg(not(windows))]
    Err("only available on Windows".into())
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn base64_matches_known_values() {
        assert_eq!(super::base64(b"Man"), "TWFu");
        assert_eq!(super::base64(b"Ma"), "TWE=");
        assert_eq!(super::base64(b"M"), "TQ==");
    }
}
