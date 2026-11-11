//! Blocking accept, bounded client lifetimes, and explicit settings/shutdown wakeups.
use super::{endpoint::Endpoint, handle, service::ServiceHost, AgentStatus};
use std::{
    io::{self, BufRead, BufReader, Read, Write},
    os::unix::net::{SocketAddr, UnixListener, UnixStream},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};
const MAX_LINE: usize = 1024 * 1024;
const MAX_CLIENTS: usize = 16;
const READ_IDLE: Duration = Duration::from_secs(30);
const WRITE_TIMEOUT: Duration = Duration::from_secs(2);
struct Client {
    stream: UnixStream,
    thread: JoinHandle<()>,
}
pub(super) struct Running {
    endpoint: Arc<Endpoint>,
    stop: Arc<AtomicBool>,
    thread: JoinHandle<()>,
    reactor: JoinHandle<()>,
    control: mpsc::Sender<()>,
    clients: Arc<Mutex<Vec<Client>>>,
}
pub(super) fn listen(
    host: Arc<ServiceHost>,
    path: PathBuf,
    version: String,
) -> io::Result<Running> {
    listen_with_accept(host, path, version, |l| l.accept())
}
pub(super) fn listen_with_accept(
    host: Arc<ServiceHost>,
    path: PathBuf,
    version: String,
    mut accept: impl FnMut(&UnixListener) -> io::Result<(UnixStream, SocketAddr)> + Send + 'static,
) -> io::Result<Running> {
    let (listener, endpoint) = Endpoint::bind(path)?;
    let endpoint = Arc::new(endpoint);
    let stop = Arc::new(AtomicBool::new(false));
    let clients = Arc::new(Mutex::new(Vec::<Client>::new()));
    let (control, receiver) = mpsc::channel();
    let enqueue = control.clone();
    let subscription = host.settings.subscribe(move || {
        let _ = enqueue.send(());
    });
    host.status
        .replace(Arc::new(AgentStatus { running: true }))
        .notification
        .notify();
    let (sessions, flag, app, wake) =
        (clients.clone(), stop.clone(), host.clone(), control.clone());
    let owned_endpoint = endpoint.clone();
    let thread = match thread::Builder::new()
        .name("kudzu-agents".into())
        .spawn(move || {
            while !flag.load(Ordering::SeqCst) {
                reap(&mut sessions.lock().unwrap());
                match accept(&listener) {
                    Ok((stream, _)) => {
                        if flag.load(Ordering::SeqCst) {
                            break;
                        }
                        let mut clients = sessions.lock().unwrap();
                        reap(&mut clients);
                        if clients.len() >= MAX_CLIENTS {
                            let _ = stream.shutdown(std::net::Shutdown::Both);
                            continue;
                        }
                        if stream
                            .set_nonblocking(false)
                            .and_then(|_| stream.set_read_timeout(Some(READ_IDLE)))
                            .and_then(|_| stream.set_write_timeout(Some(WRITE_TIMEOUT)))
                            .is_err()
                        {
                            continue;
                        }
                        let Ok(control) = stream.try_clone() else {
                            continue;
                        };
                        let (host, stop, version) = (app.clone(), flag.clone(), version.clone());
                        if let Ok(thread) = thread::Builder::new()
                            .name("kudzu-agent-client".into())
                            .spawn(move || serve(host, stream, stop, version))
                        {
                            clients.push(Client {
                                stream: control,
                                thread,
                            });
                        }
                    }
                    Err(e)
                        if matches!(
                            e.kind(),
                            io::ErrorKind::Interrupted
                                | io::ErrorKind::ConnectionAborted
                                | io::ErrorKind::WouldBlock
                        ) || matches!(e.raw_os_error(), Some(23 | 24)) =>
                    {
                        thread::sleep(Duration::from_millis(25))
                    }
                    Err(_) => break,
                }
            }
            owned_endpoint.unpublish();
            flag.store(true, Ordering::SeqCst);
            let _ = wake.send(());
            app.status
                .replace(Arc::new(AgentStatus { running: false }))
                .notification
                .notify();
        }) {
        Ok(thread) => thread,
        Err(error) => {
            host.status
                .replace(Arc::new(AgentStatus { running: false }))
                .notification
                .notify();
            return Err(error);
        }
    };
    let (flag, sessions, wake, app) = (
        stop.clone(),
        clients.clone(),
        endpoint.clone(),
        host.clone(),
    );
    let reactor = match thread::Builder::new()
        .name("kudzu-agent-access".into())
        .spawn(move || {
            let _subscription = subscription;
            loop {
                if flag.load(Ordering::SeqCst) || !app.settings.get().agent_access {
                    wake.unpublish();
                    flag.store(true, Ordering::SeqCst);
                    app.status
                        .replace(Arc::new(AgentStatus { running: false }))
                        .notification
                        .notify();
                    for client in sessions.lock().unwrap().iter() {
                        let _ = client.stream.shutdown(std::net::Shutdown::Both);
                    }
                    wake.wake();
                    app.status
                        .replace(Arc::new(AgentStatus { running: false }))
                        .notification
                        .notify();
                    break;
                }
                if receiver.recv().is_err() {
                    break;
                }
            }
        }) {
        Ok(reactor) => reactor,
        Err(error) => {
            stop.store(true, Ordering::SeqCst);
            endpoint.wake();
            let _ = thread.join();
            return Err(error);
        }
    };
    Ok(Running {
        endpoint,
        stop,
        thread,
        reactor,
        control,
        clients,
    })
}
impl Running {
    pub fn is_finished(&self) -> bool {
        self.stop.load(Ordering::SeqCst) || self.thread.is_finished()
    }
    pub fn shutdown(self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = self.control.send(());
        self.endpoint.wake();
        let _ = self.reactor.join();
        let _ = self.thread.join();
        for client in self.clients.lock().unwrap().drain(..) {
            let _ = client.stream.shutdown(std::net::Shutdown::Both);
            let _ = client.thread.join();
        }
    }
}
fn serve(host: Arc<ServiceHost>, mut stream: UnixStream, stop: Arc<AtomicBool>, version: String) {
    let Ok(read) = stream.try_clone() else { return };
    let mut reader = BufReader::new(read);
    loop {
        let mut bytes = Vec::new();
        match Read::take(&mut reader, (MAX_LINE + 1) as u64).read_until(b'\n', &mut bytes) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        if bytes.len() > MAX_LINE && !bytes.ends_with(b"\n") {
            let reply = super::error(
                serde_json::Value::Null,
                -32600,
                "Request line exceeds the 1 MiB limit.",
            );
            let _ = write_frame(&mut stream, &reply.to_string());
            break;
        }
        let Ok(line) = String::from_utf8(bytes) else {
            break;
        };
        if stop.load(Ordering::SeqCst) || !host.settings.get().agent_access {
            break;
        }
        if line.trim().is_empty() {
            continue;
        }
        #[cfg(test)]
        if let Some(gate) = host.request_gate.lock().unwrap().take() {
            let _ = gate.started.send(());
            let _ = gate.release.recv();
        }
        if let Some(reply) = handle(Some(host.as_ref()), &version, &line) {
            #[cfg(test)]
            if let Some(notice) = host.before_reply.lock().unwrap().as_ref() {
                let _ = notice.send(thread::current().id());
            }
            if stop.load(Ordering::SeqCst) || !host.settings.get().agent_access {
                break;
            }
            if write_frame(&mut stream, &reply).is_err() {
                break;
            }
        }
    }
    let _ = stream.shutdown(std::net::Shutdown::Both);
}

fn reap(clients: &mut Vec<Client>) {
    let mut i = 0;
    while i < clients.len() {
        if clients[i].thread.is_finished() {
            let c = clients.swap_remove(i);
            let _ = c.thread.join();
        } else {
            i += 1;
        }
    }
}

fn write_frame(stream: &mut UnixStream, reply: &str) -> io::Result<()> {
    let deadline = std::time::Instant::now() + WRITE_TIMEOUT;
    for mut bytes in [reply.as_bytes(), b"\n".as_slice()] {
        while !bytes.is_empty() {
            let remaining = deadline
                .checked_duration_since(std::time::Instant::now())
                .ok_or(io::ErrorKind::TimedOut)?;
            stream.set_write_timeout(Some(remaining))?;
            match stream.write(bytes) {
                Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
                Ok(n) => bytes = &bytes[n..],
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
    }
    Ok(())
}
