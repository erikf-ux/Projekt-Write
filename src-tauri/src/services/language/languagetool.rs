//! LanguageTool as an add-on (section 14, D6, ADR 0026): an existing local
//! installation the user chooses; the app downloads nothing.
//!
//! - **Setup:** the folder holds `languagetool-server.jar`. Rust keeps it in
//!   `settings.json` (`languageTool`); the frontend never names a path,
//!   because Java runs what lies there.
//! - **Server:** started on demand as a child process with Java 17 or newer
//!   (`JAVA_HOME` or `java` on the path), on `127.0.0.1` and a free port,
//!   never public. It has no login, so only Rust talks to it. It ends with
//!   the app.
//! - **Check:** one request per text and language (`/v2/check`). Spelling
//!   comes from the dictionaries and style rules stay off (section 14), so
//!   matches of the kinds `misspelling` and `style` are dropped.
//! - **Failure:** the spelling check goes on; the status says why.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::Value;
use ts_rs::TS;

use super::engine::{EngineError, FindingKind, LanguageEngine, OwnWords, RawFinding};
use crate::services::settings::app::AppSettingsStore;

/// The server jar of a LanguageTool installation.
pub const SERVER_JAR: &str = "languagetool-server.jar";
/// Key in `settings.json`.
const KEY: &str = "languageTool";
/// The Java version LanguageTool needs.
pub const MIN_JAVA: u32 = 17;
const START_TIMEOUT: Duration = Duration::from_secs(120);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ToolState {
    /// Not set up, or switched off.
    Off,
    Starting,
    Running,
    Failed,
}

/// What the interface shows of the add-on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LanguageToolStatus {
    /// An installation is chosen.
    pub configured: bool,
    /// Its folder, for display.
    pub folder: Option<String>,
    pub enabled: bool,
    pub state: ToolState,
    /// Why it failed: `java` (no Java 17), `start` (the server did not
    /// come up), `stopped` (it ended).
    pub problem: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum SetupError {
    #[error("no {SERVER_JAR} in the folder")]
    NotLanguageTool,
    #[error("settings not saved")]
    NotSaved,
}

enum Server {
    Off,
    Starting,
    Running { child: Child, port: u16 },
    Failed(String),
}

pub struct LanguageTool {
    settings: Arc<Mutex<AppSettingsStore>>,
    server: Arc<Mutex<Server>>,
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The major version in the output of `java -version` (`"17.0.2"`,
/// `"21"`, old style `"1.8.0_392"`).
pub fn java_major(output: &str) -> Option<u32> {
    let start = output.find("version \"")? + "version \"".len();
    let version = output[start..].split('"').next()?;
    let mut parts = version.split(['.', '_', '-', '+']);
    let first: u32 = parts.next()?.parse().ok()?;
    if first == 1 {
        parts.next()?.parse().ok()
    } else {
        Some(first)
    }
}

fn java_command() -> PathBuf {
    std::env::var_os("JAVA_HOME")
        .map(|home| {
            PathBuf::from(home)
                .join("bin")
                .join(if cfg!(windows) { "java.exe" } else { "java" })
        })
        .filter(|path| path.is_file())
        .unwrap_or_else(|| PathBuf::from("java"))
}

fn free_port() -> std::io::Result<u16> {
    let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))?;
    Ok(listener.local_addr()?.port())
}

/// Whether a folder holds a LanguageTool server.
pub fn is_installation(folder: &Path) -> bool {
    let jar = folder.join(SERVER_JAR);
    jar.symlink_metadata().is_ok_and(|meta| meta.is_file())
}

impl LanguageTool {
    pub fn new(settings: Arc<Mutex<AppSettingsStore>>) -> Self {
        Self {
            settings,
            server: Arc::new(Mutex::new(Server::Off)),
        }
    }

    /// The chosen folder and the switch.
    fn setup(&self) -> (Option<PathBuf>, bool) {
        let settings = lock(&self.settings);
        let entry = settings.get().other.get(KEY);
        let folder = entry
            .and_then(|e| e.get("folder"))
            .and_then(Value::as_str)
            .map(PathBuf::from);
        let enabled = entry
            .and_then(|e| e.get("enabled"))
            .and_then(Value::as_bool)
            .unwrap_or(true);
        (folder, enabled)
    }

    fn store(&self, folder: Option<&Path>, enabled: bool) -> Result<(), SetupError> {
        let value = folder.map(
            |folder| serde_json::json!({ "folder": folder.to_string_lossy(), "enabled": enabled }),
        );
        lock(&self.settings)
            .update(|settings| match value {
                Some(value) => {
                    settings.other.insert(KEY.to_owned(), value);
                }
                None => {
                    settings.other.remove(KEY);
                }
            })
            .map_err(|_| SetupError::NotSaved)
    }

    pub fn status(&self) -> LanguageToolStatus {
        let (folder, enabled) = self.setup();
        let (state, problem) = match &*lock(&self.server) {
            Server::Off => (ToolState::Off, None),
            Server::Starting => (ToolState::Starting, None),
            Server::Running { .. } => (ToolState::Running, None),
            Server::Failed(problem) => (ToolState::Failed, Some(problem.clone())),
        };
        LanguageToolStatus {
            configured: folder.is_some(),
            folder: folder.map(|f| f.to_string_lossy().into_owned()),
            enabled,
            state,
            problem,
        }
    }

    /// Uses the installation in `folder` from now on.
    pub fn configure(&self, folder: &Path) -> Result<LanguageToolStatus, SetupError> {
        if !is_installation(folder) {
            return Err(SetupError::NotLanguageTool);
        }
        self.stop();
        self.store(Some(folder), true)?;
        Ok(self.status())
    }

    pub fn set_enabled(&self, enabled: bool) -> Result<LanguageToolStatus, SetupError> {
        let (folder, _) = self.setup();
        if folder.is_some() {
            self.store(folder.as_deref(), enabled)?;
        }
        if !enabled {
            self.stop();
        }
        Ok(self.status())
    }

    /// Forgets the installation (the files stay where they are).
    pub fn forget(&self) -> Result<LanguageToolStatus, SetupError> {
        self.stop();
        self.store(None, false)?;
        Ok(self.status())
    }

    /// Starts the server in the background when it is set up, switched on
    /// and not running; a failed start is tried again on the next call.
    pub fn ensure_started(&self) {
        let (Some(folder), true) = self.setup() else {
            return;
        };
        {
            let mut server = lock(&self.server);
            if matches!(*server, Server::Starting | Server::Running { .. }) {
                return;
            }
            *server = Server::Starting;
        }
        let server = Arc::clone(&self.server);
        std::thread::spawn(move || {
            let outcome = start(&folder);
            let mut slot = lock(&server);
            // Stopped meanwhile: do not keep what came up.
            if !matches!(*slot, Server::Starting) {
                if let Ok((mut child, _)) = outcome {
                    let _ = child.kill();
                }
                return;
            }
            *slot = match outcome {
                Ok((child, port)) => Server::Running { child, port },
                Err(problem) => {
                    log::warn!("LanguageTool did not start: {problem}");
                    Server::Failed(problem)
                }
            };
        });
    }

    /// Ends the server (app exit, switched off, other installation).
    pub fn stop(&self) {
        let mut server = lock(&self.server);
        if let Server::Running { child, .. } = &mut *server {
            let _ = child.kill();
            let _ = child.wait();
        }
        *server = Server::Off;
    }

    fn port(&self) -> Option<u16> {
        let mut server = lock(&self.server);
        if let Server::Running { child, port } = &mut *server {
            // Ended on its own: the status says so.
            if child.try_wait().ok().flatten().is_some() {
                *server = Server::Failed("stopped".to_owned());
                return None;
            }
            return Some(*port);
        }
        None
    }

    fn mark_failed(&self) {
        let mut server = lock(&self.server);
        if let Server::Running { child, .. } = &mut *server {
            let _ = child.kill();
        }
        *server = Server::Failed("stopped".to_owned());
    }
}

/// Checks Java, starts the server and waits until it answers.
fn start(folder: &Path) -> Result<(Child, u16), String> {
    let java = java_command();
    let version = Command::new(&java)
        .arg("-version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|_| "java".to_owned())?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&version.stderr),
        String::from_utf8_lossy(&version.stdout)
    );
    if java_major(&text).is_none_or(|major| major < MIN_JAVA) {
        return Err("java".to_owned());
    }
    let port = free_port().map_err(|_| "start".to_owned())?;
    let mut child = Command::new(&java)
        .current_dir(folder)
        .arg("-cp")
        .arg(folder.join(SERVER_JAR))
        .arg("org.languagetool.server.HTTPServer")
        .arg("--port")
        .arg(port.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "start".to_owned())?;
    let started = Instant::now();
    while started.elapsed() < START_TIMEOUT {
        if child.try_wait().ok().flatten().is_some() {
            return Err("start".to_owned());
        }
        if request(port, "GET", "/v2/languages", None).is_ok() {
            return Ok((child, port));
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    let _ = child.kill();
    Err("start".to_owned())
}

/// `application/x-www-form-urlencoded`.
fn form_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'*' => {
                out.push(char::from(byte))
            }
            b' ' => out.push('+'),
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// A plain HTTP/1.1 request to the local server; the body of a 200 answer.
fn request(port: u16, method: &str, path: &str, body: Option<&str>) -> std::io::Result<String> {
    let address = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port);
    let mut stream = TcpStream::connect_timeout(&address.into(), REQUEST_TIMEOUT)?;
    stream.set_read_timeout(Some(REQUEST_TIMEOUT))?;
    stream.set_write_timeout(Some(REQUEST_TIMEOUT))?;
    let body = body.unwrap_or("");
    let head = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\
         Content-Type: application/x-www-form-urlencoded; charset=UTF-8\r\nContent-Length: {}\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body.as_bytes())?;
    let mut reader = BufReader::new(stream);
    let mut status = String::new();
    reader.read_line(&mut status)?;
    if status.split_whitespace().nth(1) != Some("200") {
        return Err(std::io::Error::other(format!("status {}", status.trim())));
    }
    let mut chunked = false;
    let mut length: Option<usize> = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 || line.trim().is_empty() {
            break;
        }
        let lower = line.to_ascii_lowercase();
        if lower.starts_with("transfer-encoding:") && lower.contains("chunked") {
            chunked = true;
        } else if let Some(value) = lower.strip_prefix("content-length:") {
            length = value.trim().parse().ok();
        }
    }
    let mut data = Vec::new();
    if chunked {
        loop {
            let mut size = String::new();
            reader.read_line(&mut size)?;
            let size = usize::from_str_radix(size.trim().split(';').next().unwrap_or("0"), 16)
                .map_err(std::io::Error::other)?;
            if size == 0 {
                break;
            }
            let mut chunk = vec![0; size];
            reader.read_exact(&mut chunk)?;
            data.extend_from_slice(&chunk);
            let mut end = String::new();
            reader.read_line(&mut end)?;
        }
    } else if let Some(length) = length {
        data.resize(length, 0);
        reader.read_exact(&mut data)?;
    } else {
        reader.read_to_end(&mut data)?;
    }
    String::from_utf8(data).map_err(std::io::Error::other)
}

/// Byte offset of a UTF-16 offset (LanguageTool counts Java chars).
fn byte_offset(text: &str, units: usize) -> Option<usize> {
    let mut counted = 0;
    for (offset, c) in text.char_indices() {
        if counted >= units {
            return Some(offset);
        }
        counted += c.len_utf16();
    }
    (counted >= units).then_some(text.len())
}

/// The findings of a `/v2/check` answer, without spelling and style.
pub fn parse_matches(text: &str, answer: &str) -> Result<Vec<RawFinding>, EngineError> {
    let value: Value =
        serde_json::from_str(answer).map_err(|e| EngineError::Unavailable(e.to_string()))?;
    let matches = value
        .get("matches")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut found = Vec::new();
    for item in matches {
        let rule = item.get("rule").cloned().unwrap_or(Value::Null);
        let issue = rule.get("issueType").and_then(Value::as_str).unwrap_or("");
        let category = rule
            .get("category")
            .and_then(|c| c.get("id"))
            .and_then(Value::as_str)
            .unwrap_or("");
        if issue == "misspelling" || issue == "style" || category == "TYPOS" || category == "STYLE"
        {
            continue;
        }
        let kind = match (issue, category) {
            (_, "PUNCTUATION" | "TYPOGRAPHY") | ("typographical" | "whitespace", _) => {
                FindingKind::Punctuation
            }
            (_, "CONFUSED_WORDS" | "SEMANTICS") => FindingKind::Context,
            _ => FindingKind::Grammar,
        };
        let offset = item.get("offset").and_then(Value::as_u64).unwrap_or(0) as usize;
        let length = item.get("length").and_then(Value::as_u64).unwrap_or(0) as usize;
        let (Some(start), Some(end)) = (
            byte_offset(text, offset),
            byte_offset(text, offset + length),
        ) else {
            continue;
        };
        if end <= start {
            continue;
        }
        let replacements = item
            .get("replacements")
            .and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .filter_map(|r| r.get("value").and_then(Value::as_str))
                    .take(5)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        found.push(RawFinding {
            start,
            end,
            kind,
            rule: format!(
                "lt:{}",
                rule.get("id").and_then(Value::as_str).unwrap_or("?")
            ),
            message: item
                .get("message")
                .and_then(Value::as_str)
                .map(str::to_owned),
            replacements,
        });
    }
    Ok(found)
}

impl LanguageEngine for LanguageTool {
    fn name(&self) -> &'static str {
        "LanguageTool"
    }

    fn check(
        &self,
        text: &str,
        language: &str,
        _own: &OwnWords,
    ) -> Result<Vec<RawFinding>, EngineError> {
        let port = self
            .port()
            .ok_or_else(|| EngineError::Unavailable("not running".to_owned()))?;
        let body = format!(
            "language={}&text={}",
            form_encode(language),
            form_encode(text)
        );
        match request(port, "POST", "/v2/check", Some(&body)) {
            Ok(answer) => parse_matches(text, &answer),
            Err(error) => {
                // A language it does not know answers 400: no findings.
                if error.to_string().contains("status") {
                    return Ok(Vec::new());
                }
                self.mark_failed();
                Err(EngineError::Unavailable(error.to_string()))
            }
        }
    }
}

impl Drop for LanguageTool {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_versions_are_read_in_old_and_new_style() {
        assert_eq!(
            java_major("openjdk version \"17.0.2\" 2022-01-18"),
            Some(17)
        );
        assert_eq!(java_major("openjdk version \"21\" 2023-09-19"), Some(21));
        assert_eq!(java_major("java version \"1.8.0_392\""), Some(8));
        assert_eq!(java_major("nothing"), None);
    }

    #[test]
    fn utf_16_offsets_become_byte_offsets() {
        let text = "😀 Größe";
        assert_eq!(byte_offset(text, 0), Some(0));
        assert_eq!(byte_offset(text, 3), Some(5));
        assert_eq!(byte_offset(text, 8), Some(text.len()));
        assert_eq!(byte_offset(text, 9), None);
    }

    #[test]
    fn matches_without_spelling_and_style_are_findings() {
        let text = "😀 Er gehen nach Hause , oder?";
        let answer = r#"{"matches":[
            {"message":"Verb passt nicht.","offset":6,"length":5,"replacements":[{"value":"geht"}],
             "rule":{"id":"DE_AGREEMENT","issueType":"grammar","category":{"id":"GRAMMAR"}}},
            {"message":"Leerzeichen vor Komma.","offset":22,"length":2,"replacements":[{"value":","}],
             "rule":{"id":"COMMA_WHITESPACE","issueType":"whitespace","category":{"id":"TYPOGRAPHY"}}},
            {"message":"Tippfehler","offset":3,"length":2,"replacements":[],
             "rule":{"id":"GERMAN_SPELLER_RULE","issueType":"misspelling","category":{"id":"TYPOS"}}},
            {"message":"Stil","offset":3,"length":2,"replacements":[],
             "rule":{"id":"X","issueType":"style","category":{"id":"STYLE"}}}
        ]}"#;
        let found = parse_matches(text, answer).unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!(&text[found[0].start..found[0].end], "gehen");
        assert_eq!(found[0].kind, FindingKind::Grammar);
        assert_eq!(found[0].rule, "lt:DE_AGREEMENT");
        assert_eq!(found[0].replacements, ["geht"]);
        assert_eq!(&text[found[1].start..found[1].end], " ,");
        assert_eq!(found[1].kind, FindingKind::Punctuation);
    }

    #[test]
    fn the_http_client_talks_to_a_local_server() {
        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            for chunked in [false, true] {
                let (mut stream, _) = listener.accept().unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut length = 0;
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        length = value.trim().parse().unwrap();
                    }
                    if line.trim().is_empty() {
                        break;
                    }
                }
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                assert!(
                    String::from_utf8(body)
                        .unwrap()
                        .contains("text=Gr%C3%BC%C3%9Fe+hier")
                );
                let answer = "{\"matches\":[]}";
                if chunked {
                    write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{answer}\r\n0\r\n\r\n",
                        answer.len()
                    )
                    .unwrap();
                } else {
                    write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{answer}",
                        answer.len()
                    )
                    .unwrap();
                }
            }
        });
        for _ in 0..2 {
            let body = format!("language=de-DE&text={}", form_encode("Grüße hier"));
            let answer = request(port, "POST", "/v2/check", Some(&body)).unwrap();
            assert_eq!(answer, "{\"matches\":[]}");
        }
        server.join().unwrap();
    }

    #[test]
    fn a_folder_without_the_server_is_no_installation() {
        let folder = tempfile::tempdir().unwrap();
        assert!(!is_installation(folder.path()));
        std::fs::write(folder.path().join(SERVER_JAR), b"jar").unwrap();
        assert!(is_installation(folder.path()));
    }
}
