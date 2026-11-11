//! Stdio bridge; paths and version come from composition, never ambient HOME.
use std::{
    io::{self, BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    path::Path,
    time::Duration,
};
const REPLY_TIMEOUT: Duration = Duration::from_secs(30);
pub fn run(input: impl BufRead, output: impl Write, path: &Path, version: &str) -> io::Result<()> {
    run_with_timeout(input, output, path, version, REPLY_TIMEOUT)
}
fn run_with_timeout(
    input: impl BufRead,
    mut output: impl Write,
    path: &Path,
    version: &str,
    timeout: Duration,
) -> io::Result<()> {
    let mut conn = None;
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let reply = match relay_with_timeout(&mut conn,path,&line,timeout) {
            Ok(reply)=>reply,
            Err(RelayFailure::Offline)=>super::handle(None,version,&line),
            Err(RelayFailure::TimedOut)=>super::expects_reply(&line).then(|| {
                let id=serde_json::from_str::<serde_json::Value>(&line).ok().and_then(|v|v.get("id").cloned()).unwrap_or(serde_json::Value::Null);
                super::error(id,-32000,"Kudzu didn't answer in time. The request may have completed; check before trying again.").to_string()
            }),
        };
        if let Some(reply) = reply {
            writeln!(output, "{reply}")?;
            output.flush()?;
        }
    }
    Ok(())
}
#[derive(Debug)]
enum RelayFailure {
    Offline,
    TimedOut,
}
type Conn = (BufReader<UnixStream>, UnixStream);

/// Sends one message to the app. `None` when it couldn't be delivered; `Some(None)` when it was
/// and no reply is due.
#[cfg(test)]
fn relay(conn: &mut Option<Conn>, path: &Path, line: &str) -> Option<Option<String>> {
    relay_with_timeout(conn, path, line, REPLY_TIMEOUT).ok()
}
fn relay_with_timeout(
    conn: &mut Option<Conn>,
    path: &Path,
    line: &str,
    timeout: Duration,
) -> Result<Option<String>, RelayFailure> {
    // A connection from before Kudzu restarted fails once; reconnect and try again.
    for _ in 0..2 {
        if conn.is_none() {
            let stream = UnixStream::connect(path).map_err(|_| RelayFailure::Offline)?;
            stream
                .set_read_timeout(Some(timeout))
                .map_err(|_| RelayFailure::Offline)?;
            stream
                .set_write_timeout(Some(timeout))
                .map_err(|_| RelayFailure::Offline)?;
            *conn = Some((
                BufReader::new(stream.try_clone().map_err(|_| RelayFailure::Offline)?),
                stream,
            ));
        }
        let (reader, writer) = conn.as_mut().ok_or(RelayFailure::Offline)?;
        let written = writeln!(writer, "{line}").and_then(|_| writer.flush());
        if written.as_ref().is_err_and(|error| {
            matches!(
                error.kind(),
                io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
            )
        }) {
            *conn = None;
            return Err(RelayFailure::TimedOut);
        }
        if written.is_ok() {
            if !super::expects_reply(line) {
                return Ok(None);
            }
            let mut buf = String::new();
            match reader.read_line(&mut buf) {
                Ok(n) if n > 0 => return Ok(Some(buf.trim_end().to_string())),
                // The request may have run; don't send it twice.
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) =>
                {
                    *conn = None;
                    return Err(RelayFailure::TimedOut);
                }
                _ => {}
            }
        }
        *conn = None;
    }
    Err(RelayFailure::Offline)
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
        let path = super::super::test_support::socket_path(&dir);
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
        let path = super::super::test_support::socket_path(&dir);
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
        let path = super::super::test_support::socket_path(&dir);
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

#[cfg(test)]
mod timeout_tests {
    use super::*;
    #[test]
    fn production_timeout_reports_slow_app_without_replaying() {
        let dir = super::super::test_support::directory();
        let path = super::super::test_support::socket_path(&dir);
        let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            assert!(line.contains("create_snippet"));
            line.clear();
            assert_eq!(reader.read_line(&mut line).unwrap(), 0);
            listener.set_nonblocking(true).unwrap();
            assert_eq!(
                listener.accept().unwrap_err().kind(),
                io::ErrorKind::WouldBlock
            );
        });
        let mut out = vec![];
        run_with_timeout(
            io::Cursor::new(
                "{\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"create_snippet\"}}\n",
            ),
            &mut out,
            &path,
            "1",
            Duration::from_millis(50),
        )
        .unwrap();
        server.join().unwrap();
        std::fs::remove_dir_all(dir).unwrap();
        let result = String::from_utf8(out).unwrap();
        assert!(result.contains("didn't answer in time"));
        assert!(!result.contains("isn't running"));
    }
}
