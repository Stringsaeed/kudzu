//! Owner-only Unix socket and owned client lifetimes.
use super::{handle, service::ServiceHost};
use std::{
    fs,
    io::{self, BufRead, BufReader, Write},
    os::unix::{
        fs::{FileTypeExt, PermissionsExt},
        net::{UnixListener, UnixStream},
    },
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};
struct Client {
    stream: UnixStream,
    thread: JoinHandle<()>,
}
pub(super) struct Running {
    path: PathBuf,
    stop: Arc<AtomicBool>,
    thread: JoinHandle<()>,
    clients: Arc<Mutex<Vec<Client>>>,
}
pub(super) fn listen(
    host: Arc<ServiceHost>,
    path: PathBuf,
    version: String,
) -> io::Result<Running> {
    if let Ok(meta) = fs::symlink_metadata(&path) {
        if !meta.file_type().is_socket() || UnixStream::connect(&path).is_ok() {
            return Err(io::Error::new(
                io::ErrorKind::AddrInUse,
                "Another endpoint owns the agent socket.",
            ));
        }
        fs::remove_file(&path)?;
    }
    let listener = UnixListener::bind(&path)?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    listener.set_nonblocking(true)?;
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let clients = Arc::new(Mutex::new(Vec::<Client>::new()));
    let sessions = clients.clone();
    let thread = thread::Builder::new()
        .name("kudzu-agents".into())
        .spawn(move || {
            while !flag.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let Ok(control) = stream.try_clone() else {
                            continue;
                        };
                        let host = host.clone();
                        let stop = flag.clone();
                        let version = version.clone();
                        if let Ok(thread) = thread::Builder::new()
                            .name("kudzu-agent-client".into())
                            .spawn(move || serve(host, stream, stop, version))
                        {
                            let mut sessions = sessions.lock().unwrap();
                            sessions.retain_mut(|c| {
                                if c.thread.is_finished() {
                                    let _ = c.stream.shutdown(std::net::Shutdown::Both);
                                    false
                                } else {
                                    true
                                }
                            });
                            sessions.push(Client {
                                stream: control,
                                thread,
                            });
                        }
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10))
                    }
                    Err(_) => break,
                }
            }
        })?;
    Ok(Running {
        path,
        stop,
        thread,
        clients,
    })
}
impl Running {
    pub fn shutdown(self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = self.thread.join();
        for c in self.clients.lock().unwrap().drain(..) {
            let _ = c.stream.shutdown(std::net::Shutdown::Both);
            let _ = c.thread.join();
        }
        let _ = fs::remove_file(self.path);
    }
}
fn serve(host: Arc<ServiceHost>, mut stream: UnixStream, stop: Arc<AtomicBool>, version: String) {
    let Ok(read) = stream.try_clone() else { return };
    for line in BufReader::new(read).lines() {
        let Ok(line) = line else { break };
        if stop.load(Ordering::SeqCst) || !host.settings.get().agent_access {
            break;
        }
        if line.trim().is_empty() {
            continue;
        }
        if let Some(reply) = handle(Some(host.as_ref()), &version, &line) {
            if stop.load(Ordering::SeqCst) || !host.settings.get().agent_access {
                break;
            }
            if writeln!(stream, "{reply}")
                .and_then(|_| stream.flush())
                .is_err()
            {
                break;
            }
        }
    }
    let _ = stream.shutdown(std::net::Shutdown::Both);
}
