//! The fleet update hold (`_orchestra/youtube-stack/update-hold/CONTRACT.md`): before an install
//! the app holds new work, settles what is already running inside a bounded window, the update
//! happens in that gap, and the app takes work again by itself. The pause lives in memory only,
//! so an aborted install, a missed release or a restart can never leave the app refusing work.

use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, Instant},
};

use tauri::{AppHandle, Emitter, Manager};

pub const HOLD_PATH: &str = "/api/maintenance/hold";
const HOLD_TOKEN_ENV: &str = "KHU_HOLD_TOKEN_FILE";
/// This app's row in the contract's port table. Fixed, because the signed feed names it as a
/// literal URL; `DISPLAYS_STUDIO_HOLD_PORT` exists only so a test instance avoids the live port.
/// 0.1.2 used 8649 and Wire Studio already served that port since its 0.1.0, so whichever app
/// started second could not be held.
const HOLD_PORT: u16 = 8650;
const TIMEOUT_DEFAULT_S: u64 = 20;
const TIMEOUT_MAX_S: u64 = 30;
const PAUSE_DEFAULT_S: u64 = 120;
/// A pause is a gap, not a latch: even a wild `pause_s` ends within an hour.
const PAUSE_MAX_S: u64 = 3600;

#[derive(Default)]
pub struct Hold(Mutex<State>);

#[derive(Default)]
struct State {
    until: Option<Instant>,
    /// Bumped by every pause and resume, so an old expiry timer cannot end a newer pause.
    generation: u64,
    next_id: u64,
    running: BTreeMap<u64, String>,
}

impl State {
    fn paused(&mut self) -> bool {
        match self.until {
            Some(until) if Instant::now() < until => true,
            Some(_) => {
                self.until = None;
                false
            }
            None => false,
        }
    }

    fn seconds_left(&mut self) -> u64 {
        if !self.paused() {
            return 0;
        }
        self.until
            .map(|until| until.saturating_duration_since(Instant::now()).as_secs().max(1))
            .unwrap_or(0)
    }
}

impl Hold {
    /// Registers work, refusing it while a hold stands. The caller settles the same id when the
    /// work ends; the error is the sentence a person sees.
    pub fn claim(&self, label: &str) -> Result<u64, String> {
        let mut state = self.0.lock().unwrap();
        if state.paused() {
            let seconds = state.seconds_left();
            eprintln!("update hold: refused \"{label}\" while paused ({seconds} s left)");
            return Err(format!(
                "Displays Studio KH is holding new work while an update installs. Nothing was changed; it takes work again by itself in about {seconds} s."
            ));
        }
        state.next_id += 1;
        let id = state.next_id;
        state.running.insert(id, label.to_string());
        Ok(id)
    }

    pub fn settle(&self, id: u64) {
        self.0.lock().unwrap().running.remove(&id);
    }

    pub fn in_flight(&self) -> usize {
        self.0.lock().unwrap().running.len()
    }

    /// The work still running, oldest first, each with the id a later look can follow.
    pub fn running(&self) -> Vec<String> {
        self.0
            .lock()
            .unwrap()
            .running
            .iter()
            .map(|(id, label)| format!("{label} (action #{id})"))
            .collect()
    }

    pub fn pause(&self, seconds: u64) -> u64 {
        let mut state = self.0.lock().unwrap();
        state.until = Some(Instant::now() + Duration::from_secs(seconds));
        state.generation += 1;
        state.generation
    }

    pub fn resume(&self) {
        let mut state = self.0.lock().unwrap();
        state.until = None;
        state.generation += 1;
    }

    /// Ends a pause only if it is still the one this timer was armed for.
    fn expire(&self, generation: u64) -> bool {
        let mut state = self.0.lock().unwrap();
        if state.generation != generation {
            return false;
        }
        state.until = None;
        true
    }

    /// Waits up to the ceiling for running work to end — never for the work to finish.
    pub fn settle_window(&self, seconds: u64) -> Vec<String> {
        let deadline = Instant::now() + Duration::from_secs(seconds);
        loop {
            let left = self.running();
            let remaining = deadline.saturating_duration_since(Instant::now());
            if left.is_empty() || remaining.is_zero() {
                return left;
            }
            std::thread::sleep(remaining.min(Duration::from_millis(50)));
        }
    }
}

fn bounded(raw: Option<String>, default: u64, max: u64) -> u64 {
    raw.and_then(|value| value.trim().parse::<u64>().ok()).unwrap_or(default).min(max)
}

fn query_value(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let (name, value) = pair.split_once('=')?;
        (name == key).then(|| value.to_string())
    })
}

/// Tells the person what is happening: held, what is still finishing, resumed.
fn announce(app: &AppHandle, message: String) {
    if let Some(tray) = app.tray_by_id("displays") {
        let _ = tray.set_tooltip(Some(&message));
    }
    let _ = app.emit_to("main", "update-hold", message);
}

pub fn start_server(app: AppHandle) {
    let port = std::env::var("DISPLAYS_STUDIO_HOLD_PORT")
        .ok()
        .and_then(|value| value.trim().parse::<u16>().ok())
        .unwrap_or(HOLD_PORT);
    let listener = match TcpListener::bind(("127.0.0.1", port)) {
        Ok(listener) => listener,
        // Fail open: an app that cannot open its hold port still works; it just cannot be held.
        Err(error) => {
            eprintln!("the update hold is not listening on 127.0.0.1:{port}: {error}");
            return;
        }
    };
    eprintln!("update hold listening on http://127.0.0.1:{port}{HOLD_PATH}");
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let app = app.clone();
                    // One thread per request: a settle must never block a DELETE.
                    std::thread::spawn(move || handle(&app, stream));
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) => eprintln!("update hold: {error}"),
            }
        }
    });
}

/// POST: pause new work now, settle running work up to `timeout_s`, answer whether a restart is safe.
fn hold_request(app: &AppHandle, query: &str) -> String {
    let timeout_s = bounded(query_value(query, "timeout_s"), TIMEOUT_DEFAULT_S, TIMEOUT_MAX_S);
    let pause_s = bounded(query_value(query, "pause_s"), PAUSE_DEFAULT_S, PAUSE_MAX_S).max(1);
    let hold = app.state::<Hold>();
    let started = Instant::now();
    // The claim path closes here, before any waiting.
    let generation = hold.pause(pause_s);
    announce(
        app,
        format!("Paused for an update. Nothing new starts for up to {pause_s} seconds, then it carries on by itself."),
    );
    eprintln!("held new work for an update (paused {pause_s} s)");

    // The expiry is load-bearing: an installer killed between the hold and its release sends
    // nothing, so this is what starts the app working again.
    let timer_app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(pause_s));
        if timer_app.state::<Hold>().expire(generation) {
            announce(&timer_app, "Update hold released — Displays Studio KH is working again.".into());
            eprintln!("the update pause expired; new work is accepted again");
        }
    });

    let unsettled = hold.settle_window(timeout_s);
    let in_flight = hold.in_flight();
    let safe = unsettled.is_empty();
    let ms = started.elapsed().as_millis();
    eprintln!(
        "update hold: {} in {ms} ms (ceiling {timeout_s} s, pause {pause_s} s, {in_flight} in flight){}",
        if safe { "safe" } else { "not safe" },
        if unsettled.is_empty() {
            " — ready for updating".to_string()
        } else {
            format!(" — still in flight: {}", unsettled.join(", "))
        }
    );
    if safe {
        announce(app, format!("Paused {pause_s} s for an update — updating now."));
    } else {
        announce(app, format!("Paused for an update; {} still finishing.", unsettled.join(", ")));
    }
    serde_json::json!({
        "safe": safe,
        "unsettled": unsettled,
        "dispatch_paused_s": pause_s,
        "in_flight": in_flight,
        "waiting": 0,
    })
    .to_string()
}

/// DELETE: resume at once. Never required — the pause expires by itself.
fn resume_request(app: &AppHandle) -> String {
    let hold = app.state::<Hold>();
    hold.resume();
    announce(app, "Update hold released — Displays Studio KH is working again.".into());
    eprintln!("resumed; new work is accepted again");
    serde_json::json!({
        "safe": true,
        "unsettled": Vec::<String>::new(),
        "dispatch_paused_s": 0,
        "in_flight": hold.in_flight(),
        "waiting": 0,
    })
    .to_string()
}

fn hold_token_path() -> PathBuf {
    if let Ok(override_path) = std::env::var(HOLD_TOKEN_ENV) {
        let trimmed = override_path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    // The agent rotates this file on every start. A copy-once app credential goes stale at the
    // next rotation, so the route reads the presenter's file afresh for every request.
    PathBuf::from(std::env::var("HOME").unwrap_or_default())
        .join("Library/KHUpdater/state/hold-token")
}

/// Reads the Launcher's hold credential fresh on every request so an atomic rotation takes effect
/// without restarting the app. Symlinks and oversized/empty files fail closed.
fn read_hold_token_at(path: &Path) -> Option<String> {
    let meta = std::fs::symlink_metadata(path).ok()?;
    if meta.file_type().is_symlink() || !meta.is_file() || meta.len() > 512 {
        return None;
    }
    let raw = std::fs::read_to_string(path).ok()?;
    let token = raw.trim();
    if token.is_empty() || token.len() > 256 || token.chars().any(char::is_whitespace) {
        return None;
    }
    Some(token.to_string())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (&x, &y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn authorize(headers: &[(String, String)], token_path: &Path) -> bool {
    // Reject browser-origin requests even on loopback.
    if headers.iter().any(|(name, _)| name.eq_ignore_ascii_case("origin")) {
        return false;
    }
    let Some(expected) = read_hold_token_at(token_path) else {
        return false;
    };
    let mut bearer: Option<&str> = None;
    let mut hub_token: Option<&str> = None;
    for (name, value) in headers {
        if name.eq_ignore_ascii_case("authorization") {
            let trimmed = value.trim();
            // ASCII compare on bytes: a multibyte value must not panic on a slice boundary.
            if trimmed.len() >= 7 && trimmed.as_bytes()[..7].eq_ignore_ascii_case(b"bearer ") {
                let candidate = trimmed[7..].trim();
                if !candidate.is_empty() {
                    bearer = Some(candidate);
                }
            }
        } else if name.eq_ignore_ascii_case("x-hub-token") {
            let candidate = value.trim();
            if !candidate.is_empty() {
                hub_token = Some(candidate);
            }
        }
    }
    if bearer.is_none() && hub_token.is_none() {
        return false;
    }
    let expected_bytes = expected.as_bytes();
    let bearer_ok = bearer.map_or(true, |token| constant_time_eq(token.as_bytes(), expected_bytes));
    let hub_ok = hub_token.map_or(true, |token| constant_time_eq(token.as_bytes(), expected_bytes));
    bearer_ok && hub_ok
}

enum Dispatch {
    Hold,
    Resume,
    Error(&'static str, &'static str),
}

fn route(method: &str, path: &str, headers: &[(String, String)], token_path: &Path) -> Dispatch {
    if path != HOLD_PATH {
        return Dispatch::Error("404 Not Found", r#"{"error":"no such route"}"#);
    }
    if !authorize(headers, token_path) {
        return Dispatch::Error("401 Unauthorized", r#"{"error":"unauthorized"}"#);
    }
    match method {
        "POST" => Dispatch::Hold,
        "DELETE" => Dispatch::Resume,
        _ => Dispatch::Error("405 Method Not Allowed", r#"{"error":"use POST or DELETE"}"#),
    }
}

fn handle(app: &AppHandle, mut stream: TcpStream) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
    let Ok(reader_stream) = stream.try_clone() else { return };
    let mut reader = BufReader::new(reader_stream);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }
    let mut headers: Vec<(String, String)> = Vec::new();
    let mut total_bytes = request_line.len();
    for _ in 0..64 {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) if line.trim().is_empty() => break,
            Ok(n) => {
                total_bytes += n;
                if total_bytes > 8192 {
                    break;
                }
                if let Some((name, value)) = line.split_once(':') {
                    headers.push((name.trim().to_string(), value.trim().to_string()));
                }
            }
            Err(_) => break,
        }
    }
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("");
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let (status, body) = match route(method, path, &headers, &hold_token_path()) {
        Dispatch::Hold => ("200 OK", hold_request(app, query)),
        Dispatch::Resume => ("200 OK", resume_request(app)),
        Dispatch::Error(status, json) => (status, json.to_string()),
    };
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_app_takes_work_and_settles_it() {
        let hold = Hold::default();
        let id = hold.claim("Refreshing displays").unwrap();
        assert_eq!(hold.in_flight(), 1);
        assert_eq!(hold.running(), vec!["Refreshing displays (action #1)"]);
        hold.settle(id);
        assert_eq!(hold.in_flight(), 0);
    }

    #[test]
    fn a_held_app_refuses_new_work_in_plain_words() {
        let hold = Hold::default();
        hold.pause(60);
        let reason = hold.claim("Refreshing displays").unwrap_err();
        assert!(reason.contains("holding new work"), "{reason}");
        assert!(reason.contains("by itself"), "the reason says it ends on its own: {reason}");
    }

    #[test]
    fn a_pause_expires_by_itself() {
        let hold = Hold::default();
        hold.pause(1);
        assert!(hold.claim("x").is_err());
        std::thread::sleep(Duration::from_millis(1100));
        assert!(hold.claim("x").is_ok(), "an elapsed pause never latches");
    }

    #[test]
    fn resume_opens_at_once_and_an_old_timer_cannot_close_it_again() {
        let hold = Hold::default();
        let old = hold.pause(1);
        hold.resume();
        assert!(hold.claim("x").is_ok());
        hold.pause(60);
        assert!(!hold.expire(old), "a stale expiry must not end a newer pause");
        assert!(hold.claim("y").is_err());
    }

    #[test]
    fn the_settle_is_bounded_and_names_what_is_left() {
        let hold = std::sync::Arc::new(Hold::default());
        let id = hold.claim("Refreshing displays").unwrap();
        let worker = hold.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            worker.settle(id);
        });
        let started = Instant::now();
        let left = hold.settle_window(0);
        assert_eq!(left, vec!["Refreshing displays (action #1)"]);
        assert!(started.elapsed() < Duration::from_millis(250), "a zero ceiling never waits");
        assert!(hold.settle_window(2).is_empty(), "work that ends inside the window settles");
    }

    #[test]
    fn the_two_query_values_are_read_by_name_and_bounded() {
        assert_eq!(query_value("timeout_s=20&pause_s=120", "timeout_s"), Some("20".into()));
        assert_eq!(query_value("timeout_s=20&pause_s=120", "pause_s"), Some("120".into()));
        assert_eq!(query_value("", "pause_s"), None);
        assert_eq!(bounded(Some("99".into()), TIMEOUT_DEFAULT_S, TIMEOUT_MAX_S), 30);
        assert_eq!(bounded(Some("soon".into()), TIMEOUT_DEFAULT_S, TIMEOUT_MAX_S), 20);
        assert_eq!(bounded(None, PAUSE_DEFAULT_S, PAUSE_MAX_S), 120);
    }
}
