//! AI agent access over the Model Context Protocol.
//!
//! ```text
//! agent (Claude Code, …) ──stdio──▶ Kudzu --mcp ──agent.sock──▶ running Kudzu ──▶ library, clipboard
//! ```
//!
//! The protocol, tools and secret redaction live in `kudzu_core::agent`; this module moves
//! lines between them. The socket exists only while **Allow AI agents** is on. It's owner-only
//! (0600) inside the app's data folder, so only this user's processes can reach it, and they
//! could read the library files anyway; what the socket adds is redaction and safe writes.

use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use chrono::Local;
use kudzu_core::agent::{self, Access, Edit, Host, RenderEnv, OFFLINE_MESSAGE};
use kudzu_core::clipboard::{self as model, ClipEntry};
use kudzu_core::KudzuLibrary;
use kudzu_engine::clipboard as board;
use kudzu_engine::system;
use parking_lot::Mutex;
use serde::Serialize;
use serde_json::Value;
use tauri::AppHandle;
use uuid::Uuid;

use crate::state::{core, AppCore};

/// The socket's file name in the data folder.
pub const SOCKET: &str = "agent.sock";
pub use kudzu_core::agent::SKILL;

const PROD_ID: &str = "sh.saeed.kudzu";
const DEV_ID: &str = "sh.saeed.kudzu.dev";
/// How long the bridge waits for the app to answer one request.
const REPLY_TIMEOUT: Duration = Duration::from_secs(30);

struct Running {
    path: PathBuf,
    stop: Arc<AtomicBool>,
}

static RUNNING: Mutex<Option<Running>> = Mutex::new(None);

/// Starts or stops the socket to match **Allow AI agents**.
pub fn apply(app: &AppHandle) {
    let core = core(app);
    let (on, dir) = {
        let s = core.store.lock();
        (s.settings.agent_access, s.dir.clone())
    };
    let mut running = RUNNING.lock();
    if on && running.is_none() {
        match listen(core.clone(), dir.join(SOCKET)) {
            Ok(r) => *running = Some(r),
            Err(e) => log::warn!("agent access: couldn't open the socket: {e}"),
        }
    } else if !on {
        if let Some(r) = running.take() {
            r.shut_down();
        }
    }
}

/// Removes the socket on quit.
pub fn shut_down() {
    if let Some(r) = RUNNING.lock().take() {
        r.shut_down();
    }
}

fn listen(core: Arc<AppCore>, path: PathBuf) -> io::Result<Running> {
    if path.exists() {
        if UnixStream::connect(&path).is_ok() {
            return Err(io::Error::new(io::ErrorKind::AddrInUse, "another Kudzu is already answering agents"));
        }
        // Left over from a crash.
        fs::remove_file(&path)?;
    }
    let listener = UnixListener::bind(&path)?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let version = core.app.package_info().version.to_string();
    thread::Builder::new().name("kudzu-agent".into()).spawn(move || {
        for stream in listener.incoming() {
            if flag.load(Ordering::SeqCst) {
                break;
            }
            match stream {
                Ok(stream) => {
                    let (core, flag, version) = (core.clone(), flag.clone(), version.clone());
                    let _ = thread::Builder::new().name("kudzu-agent-client".into()).spawn(move || serve(&core, stream, &flag, &version));
                }
                Err(e) => log::warn!("agent access: {e}"),
            }
        }
    })?;
    log::info!("agent access on");
    Ok(Running { path, stop })
}

impl Running {
    fn shut_down(self) {
        self.stop.store(true, Ordering::SeqCst);
        // Wake the accept loop so it sees the flag.
        let _ = UnixStream::connect(&self.path);
        let _ = fs::remove_file(&self.path);
        log::info!("agent access off");
    }
}

fn serve(core: &Arc<AppCore>, stream: UnixStream, stop: &AtomicBool, version: &str) {
    let Ok(read) = stream.try_clone() else { return };
    let host = AppHost { core: core.clone() };
    let mut out = stream;
    for line in BufReader::new(read).lines() {
        let Ok(line) = line else { break };
        // Turning access off ends open sessions too.
        if stop.load(Ordering::SeqCst) || !core.store.lock().settings.agent_access {
            break;
        }
        if line.trim().is_empty() {
            continue;
        }
        if let Some(reply) = agent::handle(&host, version, &line) {
            if writeln!(out, "{reply}").and_then(|_| out.flush()).is_err() {
                break;
            }
        }
    }
}

struct AppHost {
    core: Arc<AppCore>,
}

impl Host for AppHost {
    fn access(&self) -> Result<Access, String> {
        let s = self.core.store.lock();
        if !s.settings.agent_access {
            return Err(OFFLINE_MESSAGE.into());
        }
        Ok(Access {
            clipboard: s.settings.agent_clipboard && s.settings.clipboard_enabled,
            write: s.settings.agent_write,
            trigger_prefix: s.settings.default_trigger_prefix.clone(),
        })
    }

    fn library(&self) -> Result<KudzuLibrary, String> {
        Ok(self.core.store.lock().library.clone())
    }

    fn edit_library(&self, edit: Edit<'_>) -> Result<Value, String> {
        let value = {
            // Held for the whole edit, so a change made in the window meanwhile isn't lost.
            let mut s = self.core.store.lock();
            let mut copy = s.library.clone();
            let value = edit(&mut copy)?;
            s.library = copy;
            value
        };
        self.core.library_changed();
        Ok(value)
    }

    fn clipboard(&self) -> Result<Vec<ClipEntry>, String> {
        Ok(self.core.clip.lock().history.entries.clone())
    }

    fn clipboard_text(&self, id: Uuid) -> Result<Option<String>, String> {
        let clip = self.core.clip.lock();
        Ok(clip.items(id).as_deref().and_then(model::text_of))
    }

    fn render_env(&self) -> RenderEnv {
        let items = board::read();
        let clipboard = (!model::is_private(&items)).then(|| model::text_of(&items)).flatten();
        RenderEnv { now: Local::now().fixed_offset(), locale: system::locale(), clipboard }
    }
}

// MARK: Bridge

/// `Kudzu --mcp [--dev]`: relays an agent's stdio to the running app, one JSON-RPC message per
/// line. While the app can't be reached it still answers, so the agent sees the tools and a
/// message saying what to turn on. Returns the exit code.
pub fn bridge(dev: bool) -> i32 {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let path = home.join("Library/Application Support").join(if dev { DEV_ID } else { PROD_ID }).join(SOCKET);
    let version = bundled_version();
    let mut conn = None;
    let mut stdout = io::stdout().lock();
    for line in io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let reply = relay(&mut conn, &path, &line).unwrap_or_else(|| agent::handle(&agent::Offline, &version, &line));
        if let Some(reply) = reply {
            if writeln!(stdout, "{reply}").and_then(|_| stdout.flush()).is_err() {
                break;
            }
        }
    }
    0
}

type Conn = (BufReader<UnixStream>, UnixStream);

/// Sends one message to the app. `None` when it couldn't be delivered; `Some(None)` when it was
/// and no reply is due.
fn relay(conn: &mut Option<Conn>, path: &Path, line: &str) -> Option<Option<String>> {
    // A connection from before Kudzu restarted fails once; reconnect and try again.
    for _ in 0..2 {
        if conn.is_none() {
            let stream = UnixStream::connect(path).ok()?;
            stream.set_read_timeout(Some(REPLY_TIMEOUT)).ok()?;
            *conn = Some((BufReader::new(stream.try_clone().ok()?), stream));
        }
        let (reader, writer) = conn.as_mut()?;
        if writeln!(writer, "{line}").and_then(|_| writer.flush()).is_ok() {
            if !agent::expects_reply(line) {
                return Some(None);
            }
            let mut buf = String::new();
            match reader.read_line(&mut buf) {
                Ok(n) if n > 0 => return Some(Some(buf.trim_end().to_string())),
                // The request may have run; don't send it twice.
                Err(e) if matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut) => {
                    *conn = None;
                    return None;
                }
                _ => {}
            }
        }
        *conn = None;
    }
    None
}

fn bundled_version() -> String {
    serde_json::from_str::<Value>(include_str!("../../package.json"))
        .ok()
        .and_then(|v| v["version"].as_str().map(String::from))
        .unwrap_or_else(|| "0.0.0".into())
}

// MARK: Setup (Settings ▸ AI agents)

fn skill_path() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    home.join(".claude/skills/kudzu/SKILL.md")
}

fn shell_quote(s: &str) -> String {
    if !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b"/._-+=:,@".contains(&b)) {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', r"'\''"))
    }
}

/// How to connect an agent to this copy of Kudzu.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSetup {
    running: bool,
    /// The MCP server name to register.
    name: String,
    command: String,
    args: Vec<String>,
    /// A ready-to-paste `claude mcp add` command.
    claude_command: String,
    /// `mcpServers` JSON for other MCP clients.
    config_json: String,
    skill_path: String,
    /// "missing", "current" or "outdated".
    skill: &'static str,
    /// The skill's folder inside the app bundle (release builds), for linking into other agents.
    bundled_skill_dir: Option<String>,
}

#[tauri::command]
pub fn agent_setup(app: AppHandle) -> AgentSetup {
    let dev = crate::is_dev(&app);
    let name = if dev { "kudzu-dev" } else { "kudzu" }.to_string();
    let command = std::env::current_exe().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
    let mut args = vec!["--mcp".to_string()];
    if dev {
        args.push("--dev".into());
    }
    let claude_command = format!("claude mcp add --scope user {name} -- {} {}", shell_quote(&command), args.join(" "));
    let config_json = serde_json::to_string_pretty(&serde_json::json!({
        "mcpServers": { &name: { "command": command, "args": args } }
    }))
    .unwrap_or_default();
    let path = skill_path();
    let skill = match fs::read_to_string(&path) {
        Ok(s) if s == SKILL => "current",
        Ok(_) => "outdated",
        Err(_) => "missing",
    };
    AgentSetup {
        running: RUNNING.lock().is_some(),
        name,
        command,
        args,
        claude_command,
        config_json,
        skill_path: path.to_string_lossy().into_owned(),
        skill,
        bundled_skill_dir: bundled_skill_dir(&app).map(|p| p.to_string_lossy().into_owned()),
    }
}

fn bundled_skill_dir(app: &AppHandle) -> Option<PathBuf> {
    use tauri::Manager;
    let dir = app.path().resource_dir().ok()?.join("skills/kudzu");
    dir.join("SKILL.md").is_file().then_some(dir)
}

/// Writes the Kudzu skill to `~/.claude/skills/kudzu/SKILL.md` and returns its path.
#[tauri::command]
pub fn install_agent_skill() -> Result<String, String> {
    let path = skill_path();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    crate::store::write_atomic(&path, SKILL.as_bytes()).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_paths_for_the_shell() {
        assert_eq!(shell_quote("/Applications/Kudzu.app/Contents/MacOS/Kudzu"), "/Applications/Kudzu.app/Contents/MacOS/Kudzu");
        assert_eq!(shell_quote("/Users/a b/Kudzu Dev.app/x"), "'/Users/a b/Kudzu Dev.app/x'");
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
    }

    #[test]
    fn relays_through_a_socket_and_falls_back_offline() {
        let dir = std::env::temp_dir().join(format!("kz-agent-{}", Uuid::new_v4().simple()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(SOCKET);
        let mut conn = None;
        let ping = r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#;
        assert_eq!(relay(&mut conn, &path, ping), None, "nothing listening");

        let listener = UnixListener::bind(&path).unwrap();
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut out = stream.try_clone().unwrap();
            for line in BufReader::new(stream).lines() {
                let line = line.unwrap();
                if let Some(reply) = agent::handle(&agent::Offline, "9", &line) {
                    writeln!(out, "{reply}").unwrap();
                }
            }
        });
        assert_eq!(relay(&mut conn, &path, r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#), Some(None));
        let reply: Value = serde_json::from_str(&relay(&mut conn, &path, ping).unwrap().unwrap()).unwrap();
        assert_eq!(reply, serde_json::json!({ "jsonrpc": "2.0", "id": 1, "result": {} }));
        drop(conn);
        server.join().unwrap();
        let _ = fs::remove_dir_all(dir);
    }
}
