//! Launch at login, with one per-user LaunchAgent and an honest answer when macOS refuses.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const LABEL: &str = "app.displaysstudiokh.desktop";
const PROGRAM: &str = "/Applications/Displays Studio KH.app/Contents/MacOS/displays-studio-kh";
const INSTALL_FIRST: &str = "Install Displays Studio KH in Applications first, then turn this on.";

#[derive(Clone, Default, serde::Serialize)]
pub struct Status {
    pub enabled: bool,
    pub error: Option<String>,
}

/// True when this user currently has the agent installed for this app.
pub fn enabled() -> bool {
    match agent_path() {
        Ok(path) => fs::read_to_string(path).map(|body| body.contains(LABEL)).unwrap_or(false),
        Err(_) => false,
    }
}

/// Install or remove the agent. Returns what is true afterwards, never what was asked for.
pub fn apply(enabled: bool) -> Status {
    apply_at(enabled, PROGRAM)
}

fn agent_path() -> Result<PathBuf, String> {
    let home = env::var_os("HOME").ok_or("Could not find your home folder.")?;
    Ok(PathBuf::from(home).join("Library/LaunchAgents").join(format!("{LABEL}.plist")))
}

fn uid() -> Result<String, String> {
    let out = Command::new("id").arg("-u").output().map_err(|e| format!("Could not check the user id: {e}"))?;
    let id = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if out.status.success() && id.parse::<u32>().is_ok() {
        Ok(id)
    } else {
        Err("Could not check the user id.".into())
    }
}

fn launchctl_message(out: &Output) -> String {
    let parts = [String::from_utf8_lossy(&out.stderr), String::from_utf8_lossy(&out.stdout)];
    let text = parts.iter().map(|part| part.trim()).filter(|part| !part.is_empty()).collect::<Vec<_>>().join(" ");
    if text.is_empty() { format!("launchctl exited with {}", out.status) } else { text }
}

fn says(message: &str, phrase: &str) -> bool {
    message.to_ascii_lowercase().contains(phrase)
}

fn xml(value: &str) -> String {
    value.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn plist(program: &str) -> String {
    format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{program}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <false/>
    <key>ProcessType</key>
    <string>Interactive</string>
</dict>
</plist>
"#, label = LABEL, program = xml(program))
}

fn apply_at(enabled: bool, program: &str) -> Status {
    if enabled && !Path::new(program).exists() {
        return Status { enabled: false, error: Some(INSTALL_FIRST.into()) };
    }
    let path = match agent_path() {
        Ok(path) => path,
        Err(e) => return Status { enabled: false, error: Some(e) },
    };
    let uid = match uid() {
        Ok(uid) => uid,
        Err(e) => return Status { enabled: false, error: Some(e) },
    };
    let path_text = path.to_string_lossy().into_owned();

    if !enabled {
        let service = format!("gui/{uid}/{LABEL}");
        match Command::new("launchctl").arg("bootout").arg(&service).output() {
            Ok(out) if !out.status.success() && !says(&launchctl_message(&out), "not loaded") => {
                return Status {
                    enabled: false,
                    error: Some(format!("The login item was not removed. launchctl said: {}", launchctl_message(&out))),
                };
            }
            Err(e) => {
                return Status { enabled: false, error: Some(format!("The login item was not removed. Could not run launchctl: {e}")) };
            }
            _ => {}
        }
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Status { enabled: false, error: Some(format!("Could not remove the login item file: {e}")) },
        }
        return Status { enabled: false, error: None };
    }

    if let Some(parent) = path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            return Status { enabled: false, error: Some(format!("Could not create the login item folder: {e}")) };
        }
    }
    if let Err(e) = fs::write(&path, plist(program)) {
        return Status { enabled: false, error: Some(format!("Could not write the login item file: {e}")) };
    }

    let domain = format!("gui/{uid}");
    match Command::new("launchctl").args(["bootstrap", domain.as_str(), path_text.as_str()]).output() {
        Ok(out) if out.status.success() || says(&launchctl_message(&out), "already bootstrapped") => {
            Status { enabled: true, error: None }
        }
        Ok(out) => Status {
            enabled: false,
            error: Some(format!("The login item was not registered. launchctl said: {}", launchctl_message(&out))),
        },
        Err(e) => Status {
            enabled: false,
            error: Some(format!("The login item was not registered. Could not run launchctl: {e}")),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plist_names_the_label_and_program() {
        let program = "/Applications/Displays Studio KH.app/Contents/MacOS/displays-studio-kh";
        let body = plist(program);
        assert!(body.contains("<string>app.displaysstudiokh.desktop</string>"));
        assert!(body.contains(&format!("<string>{program}</string>")));
        assert!(body.contains("<key>RunAtLoad</key>\n    <true/>"));
        assert!(body.contains("<key>KeepAlive</key>\n    <false/>"));
        assert!(body.contains("<key>ProcessType</key>\n    <string>Interactive</string>"));
    }

    #[test]
    fn missing_program_says_install_first_without_touching_launchd() {
        let status = apply_at(true, "/nonexistent/Displays Studio KH.app/Contents/MacOS/displays-studio-kh");
        assert!(!status.enabled);
        assert_eq!(status.error.as_deref(), Some(INSTALL_FIRST));
    }
}
