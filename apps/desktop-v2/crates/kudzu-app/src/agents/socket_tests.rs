use super::test_support::Harness;
use kudzu_core::KudzuLibrary;
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    time::Duration,
};

fn connect(h: &Harness) -> (UnixStream, BufReader<UnixStream>) {
    let stream = UnixStream::connect(&h.service.config.socket).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    (stream.try_clone().unwrap(), BufReader::new(stream))
}
fn request(out: &mut UnixStream, input: &mut BufReader<UnixStream>, message: Value) -> Value {
    writeln!(out, "{message}").unwrap();
    let mut reply = String::new();
    assert!(
        input.read_line(&mut reply).unwrap() > 0,
        "session unexpectedly ended"
    );
    serde_json::from_str(&reply).unwrap()
}
#[test]
fn blocking_clients_survive_idle_repeated_and_large_messages() {
    let h = Harness::new(KudzuLibrary::default(), vec![]);
    h.service.apply().unwrap();
    let (mut out, mut input) = connect(&h);
    std::thread::sleep(Duration::from_millis(50));
    for id in 1..4 {
        assert_eq!(
            request(&mut out, &mut input, json!({"id":id,"method":"ping"}))["id"],
            id
        );
        std::thread::sleep(Duration::from_millis(300));
    }
    let body = "ordinary paragraph with spaces. ".repeat(4000);
    let reply = request(
        &mut out,
        &mut input,
        json!({"id":4,"method":"tools/call","params":{"name":"create_snippet","arguments":{"name":"Large","content":body,"triggers":[";large"]}}}),
    );
    assert_eq!(reply["result"]["isError"], false);
    let reply = request(
        &mut out,
        &mut input,
        json!({"id":5,"method":"tools/call","params":{"name":"get_snippet","arguments":{"trigger":";large"}}}),
    );
    assert_eq!(reply["result"]["structuredContent"]["content"], body);
}

#[test]
fn oversized_lines_receive_invalid_request_and_disconnect() {
    let h = Harness::new(KudzuLibrary::default(), vec![]);
    h.service.apply().unwrap();
    let (mut out, mut input) = connect(&h);
    let _ = out.write_all(&vec![b'x'; 1024 * 1024 + 1]);
    let _ = out.write_all(b"\n");
    let mut reply = String::new();
    assert!(input.read_line(&mut reply).unwrap() > 0);
    let reply: Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(reply["error"]["code"], -32600);
    let mut extra = String::new();
    assert_eq!(input.read_line(&mut extra).unwrap_or(0), 0);
}
#[test]
fn sixteenth_client_is_last_admitted_and_finished_clients_are_reaped() {
    let h = Harness::new(KudzuLibrary::default(), vec![]);
    h.service.apply().unwrap();
    let mut clients = vec![];
    for id in 0..16 {
        let (mut out, mut input) = connect(&h);
        assert_eq!(
            request(&mut out, &mut input, json!({"id":id,"method":"ping"}))["id"],
            id
        );
        clients.push((out, input));
    }
    let (mut out, mut input) = connect(&h);
    let _ = writeln!(out, "{{\"id\":17,\"method\":\"ping\"}}");
    let mut reply = String::new();
    assert_eq!(
        input.read_line(&mut reply).unwrap_or(0),
        0,
        "seventeenth client must be refused"
    );
    let (out, _) = clients.pop().unwrap();
    let _ = out.shutdown(std::net::Shutdown::Both);
    drop(out);
    std::thread::sleep(Duration::from_millis(50));
    let (mut out, mut input) = connect(&h);
    assert_eq!(
        request(&mut out, &mut input, json!({"id":18,"method":"ping"}))["id"],
        18
    );
}
#[test]
fn stalled_reader_is_disconnected_by_write_timeout() {
    use std::io::Read;
    let body = "plain paragraph. ".repeat(100_000);
    let snippet = kudzu_core::SnippetRecord::new("Large", vec![";large".into()], body);
    let h = Harness::new(
        KudzuLibrary {
            snippets: vec![snippet],
            ..Default::default()
        },
        vec![],
    );
    let (notice, ready) = std::sync::mpsc::channel();
    *h.service.host.before_reply.lock().unwrap() = Some(notice);
    h.service.apply().unwrap();
    let (mut out, mut input) = connect(&h);
    writeln!(out,"{}",json!({"id":1,"method":"tools/call","params":{"name":"get_snippet","arguments":{"trigger":";large"}}})).unwrap();
    ready.recv_timeout(Duration::from_secs(20)).unwrap();
    std::thread::sleep(Duration::from_secs(3));
    let mut bytes = vec![];
    let result = input.read_to_end(&mut bytes);
    assert!(
        result.as_ref().map_or_else(
            |e| e.kind() == std::io::ErrorKind::ConnectionReset,
            |_| true
        ),
        "write timeout must close a stalled session: {result:?}"
    );
    assert!(
        !bytes.ends_with(b"\n"),
        "large reply must have been abandoned"
    );
}

#[test]
fn accepts_again_after_descriptor_exhaustion_and_transient_errors() {
    let h = Harness::new(KudzuLibrary::default(), vec![]);
    let mut faults = std::collections::VecDeque::from([
        std::io::Error::from_raw_os_error(24),
        std::io::Error::from_raw_os_error(23),
        std::io::ErrorKind::Interrupted.into(),
        std::io::ErrorKind::ConnectionAborted.into(),
        std::io::ErrorKind::WouldBlock.into(),
    ]);
    let running = super::socket::listen_with_accept(
        h.service.host.clone(),
        h.service.config.socket.clone(),
        "1".into(),
        move |listener| {
            if let Some(error) = faults.pop_front() {
                Err(error)
            } else {
                listener.accept()
            }
        },
    )
    .unwrap();
    std::thread::sleep(Duration::from_millis(100));
    let (mut out, mut input) = connect(&h);
    assert_eq!(
        request(&mut out, &mut input, json!({"id":1,"method":"ping"}))["id"],
        1
    );
    running.shutdown();
}

fn revoke(h: &Harness) {
    h.settings
        .replace(std::sync::Arc::new(crate::settings::Settings {
            agent_access: false,
            ..Default::default()
        }))
        .notification
        .notify();
}
#[test]
fn idle_access_revocation_closes_the_session_without_another_request() {
    let h = Harness::new(KudzuLibrary::default(), vec![]);
    h.service.apply().unwrap();
    let (mut out, mut input) = connect(&h);
    request(&mut out, &mut input, json!({"id":1,"method":"ping"}));
    revoke(&h);
    let mut reply = String::new();
    assert_eq!(input.read_line(&mut reply).unwrap(), 0);
    assert!(!h.service.status().get().running);
}
#[test]
fn shutdown_and_drop_remove_only_the_socket_they_bound() {
    use std::os::unix::net::UnixListener;
    let h = Harness::new(KudzuLibrary::default(), vec![]);
    h.service.apply().unwrap();
    let path = h.service.config.socket.clone();
    std::fs::remove_file(&path).unwrap();
    let replacement = UnixListener::bind(&path).unwrap();
    h.service.shutdown();
    assert!(
        path.exists(),
        "shutdown must preserve a replacement endpoint"
    );
    drop(replacement);
    std::fs::remove_file(path).unwrap();
}
#[test]
fn fatal_accept_failure_publishes_stopped_status() {
    let h = Harness::new(KudzuLibrary::default(), vec![]);
    let running = super::socket::listen_with_accept(
        h.service.host.clone(),
        h.service.config.socket.clone(),
        "1".into(),
        |_| Err(std::io::ErrorKind::InvalidInput.into()),
    )
    .unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while h.service.status().get().running && std::time::Instant::now() < deadline {
        std::thread::yield_now();
    }
    assert!(!h.service.status().get().running);
    running.shutdown();
}

#[test]
fn each_client_has_one_thread_and_keeps_it_for_later_requests() {
    let h = Harness::new(KudzuLibrary::default(), vec![]);
    let (notice, ready) = std::sync::mpsc::channel();
    *h.service.host.before_reply.lock().unwrap() = Some(notice);
    h.service.apply().unwrap();
    let (mut a, mut ar) = connect(&h);
    let (mut b, mut br) = connect(&h);
    request(&mut a, &mut ar, json!({"id":1,"method":"ping"}));
    let first = ready.recv_timeout(Duration::from_secs(2)).unwrap();
    request(&mut b, &mut br, json!({"id":2,"method":"ping"}));
    let second = ready.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_ne!(first, second);
    request(&mut a, &mut ar, json!({"id":3,"method":"ping"}));
    assert_eq!(ready.recv_timeout(Duration::from_secs(2)).unwrap(), first);
}
#[test]
fn drop_removes_the_bound_socket() {
    let h = Harness::new(KudzuLibrary::default(), vec![]);
    let service = super::AgentsService::new(
        h.service.config.clone(),
        h.service.host.library.clone(),
        h.service.host.clipboard.clone(),
        h.settings.watch(),
        h.service.host.clock.clone(),
        h.service.host.ids.clone(),
    );
    service.apply().unwrap();
    let path = service.config.socket.clone();
    assert!(path.exists());
    drop(service);
    assert!(!path.exists());
}
#[test]
fn large_request_and_reply_round_trip_through_real_bridge() {
    let h = Harness::new(KudzuLibrary::default(), vec![]);
    h.service.apply().unwrap();
    let body = "ordinary paragraph. ".repeat(6000);
    let messages = [
        json!({"id":1,"method":"tools/call","params":{"name":"create_snippet","arguments":{"name":"Large","content":body,"triggers":[";bridge"]}}}),
        json!({"id":2,"method":"tools/call","params":{"name":"get_snippet","arguments":{"trigger":";bridge"}}}),
    ];
    let input = messages
        .iter()
        .map(|m| format!("{m}\n"))
        .collect::<String>();
    let mut output = vec![];
    super::bridge::run(
        std::io::Cursor::new(input),
        &mut output,
        &h.service.config.socket,
        "1",
    )
    .unwrap();
    let replies: Vec<Value> = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert_eq!(replies[1]["result"]["structuredContent"]["content"], body);
}
#[test]
fn socket_binding_rejects_symlinks_and_publishes_only_private_mode() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let h = Harness::new(KudzuLibrary::default(), vec![]);
    let path = &h.service.config.socket;
    let target = h.dir.join("keep");
    std::fs::write(&target, "keep").unwrap();
    symlink(&target, path).unwrap();
    assert!(h.service.apply().is_err());
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "keep");
    std::fs::remove_file(path).unwrap();
    h.service.apply().unwrap();
    assert_eq!(
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let private = std::fs::read_dir(&h.dir)
        .unwrap()
        .flatten()
        .find(|e| e.file_name().to_string_lossy().starts_with(".a"))
        .unwrap();
    assert_eq!(
        private.metadata().unwrap().permissions().mode() & 0o777,
        0o700
    );
    h.service.shutdown();
    assert!(!path.exists());
}

#[test]
fn revoking_access_during_a_request_withholds_its_reply() {
    let h = Harness::new(KudzuLibrary::default(), vec![]);
    let (started, entered) = std::sync::mpsc::channel();
    let (release, wait) = std::sync::mpsc::channel();
    *h.service.host.request_gate.lock().unwrap() = Some(super::service::RequestGate {
        started,
        release: wait,
    });
    h.service.apply().unwrap();
    let (mut out, mut input) = connect(&h);
    writeln!(out, "{{\"id\":1,\"method\":\"ping\"}}").unwrap();
    entered.recv_timeout(Duration::from_secs(3)).unwrap();
    revoke(&h);
    let mut reply = String::new();
    let result = input.read_line(&mut reply);
    release.send(()).unwrap();
    assert_eq!(result.unwrap(), 0);
    assert!(reply.is_empty());
}
