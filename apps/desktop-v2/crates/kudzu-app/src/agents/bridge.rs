//! Stdio bridge; paths and version come from composition, never ambient HOME.
use std::{
    io::{self, BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    path::Path,
    time::Duration,
};
const REPLY_TIMEOUT: Duration = Duration::from_secs(30);
pub fn run(
    input: impl BufRead,
    mut output: impl Write,
    path: &Path,
    version: &str,
) -> io::Result<()> {
    let mut conn = None;
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let reply =
            relay(&mut conn, path, &line).unwrap_or_else(|| super::handle(None, version, &line));
        if let Some(reply) = reply {
            writeln!(output, "{reply}")?;
            output.flush()?;
        }
    }
    Ok(())
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
        if writeln!(writer, "{line}")
            .and_then(|_| writer.flush())
            .is_ok()
        {
            if !super::expects_reply(line) {
                return Some(None);
            }
            let mut buf = String::new();
            match reader.read_line(&mut buf) {
                Ok(n) if n > 0 => return Some(Some(buf.trim_end().to_string())),
                // The request may have run; don't send it twice.
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) =>
                {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::{fs, os::unix::net::UnixListener, thread};
    #[test]
    fn relays_through_a_socket_and_falls_back_offline() {
        let dir = super::super::test_support::directory();
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("agent.sock");
        let mut conn = None;
        let ping = r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#;
        assert_eq!(relay(&mut conn, &path, ping), None, "nothing listening");

        let listener = UnixListener::bind(&path).unwrap();
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut out = stream.try_clone().unwrap();
            for line in BufReader::new(stream).lines() {
                let line = line.unwrap();
                if let Some(reply) = crate::agents::handle(None, "9", &line) {
                    writeln!(out, "{reply}").unwrap();
                }
            }
        });
        assert_eq!(
            relay(
                &mut conn,
                &path,
                r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#
            ),
            Some(None)
        );
        let reply: Value =
            serde_json::from_str(&relay(&mut conn, &path, ping).unwrap().unwrap()).unwrap();
        assert_eq!(
            reply,
            serde_json::json!({ "jsonrpc": "2.0", "id": 1, "result": {} })
        );
        drop(conn);
        server.join().unwrap();
        let _ = fs::remove_dir_all(dir);
    }
}

#[cfg(test)]
mod recovery_tests {
    use super::*;
    use std::{fs, os::unix::net::UnixListener, thread};
    #[test]
    fn reconnects_once_after_a_closed_connection() {
        let dir = super::super::test_support::directory();
        let path = dir.join("agent.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let server = thread::spawn(move || {
            for attempt in 0..2 {
                let (stream, _) = listener.accept().unwrap();
                let mut writer = stream.try_clone().unwrap();
                let mut line = String::new();
                BufReader::new(stream).read_line(&mut line).unwrap();
                if attempt == 1 {
                    writeln!(writer, "{{\"id\":1,\"result\":{{}}}}").unwrap();
                }
            }
        });
        assert!(relay(&mut None, &path, r#"{"id":1,"method":"ping"}"#)
            .unwrap()
            .unwrap()
            .contains("result"));
        server.join().unwrap();
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn timeout_never_replays_a_request_and_the_next_call_reconnects() {
        assert_eq!(REPLY_TIMEOUT, Duration::from_secs(30));
        let dir = super::super::test_support::directory();
        let path = dir.join("agent.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            assert!(line.contains("create_snippet"));
            line.clear();
            assert_eq!(
                reader.read_line(&mut line).unwrap(),
                0,
                "bridge closed after timeout without replay"
            );
            let (stream, _) = listener.accept().unwrap();
            let mut writer = stream.try_clone().unwrap();
            reader = BufReader::new(stream);
            reader.read_line(&mut line).unwrap();
            assert!(line.contains("ping"));
            writeln!(writer, "{{\"id\":2,\"result\":{{}}}}").unwrap();
        });
        let stream = UnixStream::connect(&path).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_millis(50)))
            .unwrap();
        let mut conn = Some((BufReader::new(stream.try_clone().unwrap()), stream));
        assert_eq!(
            relay(
                &mut conn,
                &path,
                r#"{"id":1,"method":"tools/call","params":{"name":"create_snippet"}}"#
            ),
            None
        );
        assert!(conn.is_none());
        assert!(relay(&mut conn, &path, r#"{"id":2,"method":"ping"}"#)
            .unwrap()
            .is_some());
        drop(conn);
        server.join().unwrap();
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn offline_bridge_frames_lines_and_ignores_notifications() {
        let dir = super::super::test_support::directory();
        let mut output = vec![];
        run(io::Cursor::new("\n{\"id\":1,\"method\":\"tools/list\"}\n{\"method\":\"notifications/initialized\"}\n{\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"list_snippets\"}}\n"),&mut output,&dir.join("offline.sock"),"1").unwrap();
        let output = String::from_utf8(output).unwrap();
        assert_eq!(output.lines().count(), 2);
        assert!(output.contains("list_snippets"));
        assert!(output.contains(super::super::OFFLINE_MESSAGE));
        fs::remove_dir_all(dir).unwrap();
    }
}
