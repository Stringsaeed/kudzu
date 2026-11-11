//! User-selected local files go through file URLs, never a shell or URL parser.
use kudzu_app::{error::PortError, ports::FileOpener};
use std::path::Path;

#[derive(Default)]
pub struct WorkspaceFiles;

impl FileOpener for WorkspaceFiles {
    fn open(&self, path: &Path) -> Result<(), PortError> {
        perform(path, false)
    }
    fn reveal(&self, path: &Path) -> Result<(), PortError> {
        perform(path, true)
    }
}

#[cfg(target_os = "macos")]
fn perform(path: &Path, reveal: bool) -> Result<(), PortError> {
    use objc2::rc::autoreleasepool;
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::{NSArray, NSString, NSURL};
    let path = std::path::absolute(path).map_err(|e| PortError::Io(e.into()))?;
    if !path.exists() {
        return Err(PortError::TargetGone);
    }
    let path = path.to_str().ok_or(PortError::InvalidData)?;
    autoreleasepool(|_| {
        let url = NSURL::fileURLWithPath(&NSString::from_str(path));
        let workspace = NSWorkspace::sharedWorkspace();
        if reveal {
            workspace.activateFileViewerSelectingURLs(&NSArray::from_retained_slice(&[url]));
            Ok(()) // This AppKit API acknowledges the request, not Finder's eventual selection.
        } else if workspace.openURL(&url) {
            Ok(())
        } else {
            Err(PortError::Unavailable)
        }
    })
}

#[cfg(not(target_os = "macos"))]
fn perform(_: &Path, _: bool) -> Result<(), PortError> {
    Err(PortError::Unavailable)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_files_fail_without_opening_an_application() {
        let path = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        assert!(WorkspaceFiles.open(&path).is_err());
        assert!(WorkspaceFiles.reveal(&path).is_err());
    }
}

/// Blocking HTTP adapter, colocated with local files for S36's file ownership boundary.
pub mod fetch {
    mod proxy;
    use kudzu_app::{
        error::PortError,
        ports::{Fetch, FetchRequest},
    };
    use std::{
        io::Read,
        sync::{atomic::Ordering, mpsc},
        thread,
        time::{Duration, Instant},
    };

    #[derive(Default)]
    pub struct HttpFetch;
    impl Fetch for HttpFetch {
        fn get(
            &self,
            request: &FetchRequest,
            progress: &dyn Fn(u64, Option<u64>),
        ) -> Result<Vec<u8>, PortError> {
            get(request, progress, true)
        }
    }
    pub(super) fn get(
        request: &FetchRequest,
        progress: &dyn Fn(u64, Option<u64>),
        https_only: bool,
    ) -> Result<Vec<u8>, PortError> {
        get_inner(request, progress, https_only, https_only)
    }
    fn get_inner(
        request: &FetchRequest,
        progress: &dyn Fn(u64, Option<u64>),
        https_only: bool,
        use_proxy: bool,
    ) -> Result<Vec<u8>, PortError> {
        check(request)?;
        if https_only
            && request
                .url
                .parse::<ureq::http::Uri>()
                .ok()
                .and_then(|u| u.scheme_str().map(str::to_owned))
                .as_deref()
                != Some("https")
        {
            return Err(PortError::Unavailable);
        }
        if request.timeout.is_zero() {
            return Err(PortError::TimedOut);
        }
        let started = Instant::now();
        let config = ureq::Agent::config_builder()
            .https_only(https_only)
            .timeout_global(Some(request.timeout))
            .max_redirects(0)
            .http_status_as_error(false)
            .accept_encoding(ureq::config::AutoHeaderValue::None)
            .tls_config(
                ureq::tls::TlsConfig::builder()
                    .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                    .build(),
            )
            .build();
        let agent: ureq::Agent = config.into();
        let _cancellation = CancelWatch::start(agent.clone(), request.cancel.clone())?;
        let mut url = request.url.clone();
        let mut response = None;
        for _ in 0..=10 {
            check(request)?;
            let proxy = if use_proxy {
                proxy::resolve(request, &url, started)?
            } else {
                None
            };
            let remaining = request
                .timeout
                .checked_sub(started.elapsed())
                .ok_or(PortError::TimedOut)?;
            let res = agent
                .get(&url)
                .config()
                .proxy(proxy)
                .timeout_global(Some(remaining))
                .build()
                .call()
                .map_err(|e| {
                    if request.cancel.load(Ordering::Acquire) {
                        PortError::Cancelled
                    } else if matches!(e, ureq::Error::Timeout(_)) {
                        PortError::TimedOut
                    } else {
                        PortError::Unavailable
                    }
                })?;
            if res.status().is_redirection() {
                let location = res
                    .headers()
                    .get("location")
                    .and_then(|v| v.to_str().ok())
                    .ok_or(PortError::InvalidData)?;
                // ureq resolves relative Location values when redirects are enabled. We resolve
                // through its HTTP URI representation and check every hop before issuing it.
                url = redirect(&url, location)?;
            } else if res.status().is_success() {
                response = Some(res);
                break;
            } else {
                return Err(PortError::Unavailable);
            }
        }
        let mut response = response.ok_or(PortError::Unavailable)?;
        check(request)?;
        let total = response
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok());
        if total.is_some_and(|n| n > request.max_bytes as u64) {
            return Err(PortError::TooLarge);
        }
        progress(0, total);
        let mut output = Vec::new();
        let mut buffer = [0; 16 * 1024];
        let mut reader = response.body_mut().as_reader();
        loop {
            check(request)?;
            if started.elapsed() >= request.timeout {
                return Err(PortError::TimedOut);
            }
            let count = reader.read(&mut buffer).map_err(|e| {
                if request.cancel.load(Ordering::Acquire) {
                    PortError::Cancelled
                } else if started.elapsed() >= request.timeout
                    || e.kind() == std::io::ErrorKind::TimedOut
                    || e.get_ref()
                        .and_then(|e| e.downcast_ref::<ureq::Error>())
                        .is_some_and(|e| matches!(e, ureq::Error::Timeout(_)))
                {
                    PortError::TimedOut
                } else {
                    PortError::InvalidData
                }
            })?;
            check(request)?;
            if count == 0 {
                break;
            }
            if count > request.max_bytes.saturating_sub(output.len()) {
                return Err(PortError::TooLarge);
            }
            output.extend_from_slice(&buffer[..count]);
            progress(output.len() as u64, total);
        }
        Ok(output)
    }
    // ureq 3.5's shutdown closes active sockets, including a stalled body read. The
    // watcher is stopped and joined on every exit; no cancelled download thread survives.
    struct CancelWatch {
        stop: mpsc::SyncSender<()>,
        thread: Option<thread::JoinHandle<()>>,
    }
    impl CancelWatch {
        fn start(agent: ureq::Agent, cancel: kudzu_app::ports::Cancel) -> Result<Self, PortError> {
            let (stop, receiver) = mpsc::sync_channel(1);
            let thread = thread::Builder::new()
                .name("kudzu-fetch-cancel".into())
                .spawn(move || loop {
                    if cancel.load(Ordering::Acquire) {
                        agent.shutdown();
                        break;
                    }
                    match receiver.recv_timeout(Duration::from_millis(20)) {
                        Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                    }
                })
                .map_err(|e| PortError::Io(e.into()))?;
            Ok(Self {
                stop,
                thread: Some(thread),
            })
        }
    }
    impl Drop for CancelWatch {
        fn drop(&mut self) {
            let _ = self.stop.send(());
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }
    fn check(request: &FetchRequest) -> Result<(), PortError> {
        if request.cancel.load(Ordering::Acquire) {
            Err(PortError::Cancelled)
        } else {
            Ok(())
        }
    }
    fn redirect(base: &str, location: &str) -> Result<String, PortError> {
        let base: ureq::http::Uri = base.parse().map_err(|_| PortError::InvalidData)?;
        let target = if location.starts_with('?') {
            format!(
                "{}://{}{}{}",
                base.scheme_str().ok_or(PortError::InvalidData)?,
                base.authority().ok_or(PortError::InvalidData)?,
                base.path(),
                location
            )
        } else if location
            .parse::<ureq::http::Uri>()
            .map_err(|_| PortError::InvalidData)?
            .scheme()
            .is_some()
        {
            location.to_owned()
        } else if location.starts_with("//") {
            format!("https:{location}")
        } else {
            let origin = format!(
                "{}://{}",
                base.scheme_str().ok_or(PortError::InvalidData)?,
                base.authority().ok_or(PortError::InvalidData)?
            );
            if location.starts_with('/') {
                format!("{origin}{location}")
            } else {
                format!(
                    "{origin}{}/{}",
                    base.path().rsplit_once('/').map_or("", |(dir, _)| dir),
                    location
                )
            }
        };
        let uri: ureq::http::Uri = target.parse().map_err(|_| PortError::InvalidData)?;
        if uri.scheme_str() != Some("https") {
            return Err(PortError::InvalidData);
        }
        Ok(target)
    }
    #[cfg(test)]
    mod review_tests {

        use super::*;
        use std::{
            io::{Read, Write},
            net::TcpListener,
            process::Command,
        };
        #[test]
        fn requests_send_no_accept_encoding_and_preserve_the_body_and_length() {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let task = thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut header = Vec::new();
                let mut byte = [0];
                while !header.ends_with(b"\r\n\r\n") {
                    stream.read_exact(&mut byte).unwrap();
                    header.push(byte[0]);
                }
                assert!(!String::from_utf8(header)
                    .unwrap()
                    .to_ascii_lowercase()
                    .contains("accept-encoding:"));
                stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nContent-Encoding: gzip\r\n\r\nexact").unwrap();
            });
            let request = FetchRequest::new(
                format!("http://{address}/archive"),
                Duration::from_secs(2),
                64,
            );
            let progress = std::sync::Mutex::new(Vec::new());
            assert_eq!(
                get(
                    &request,
                    &|n, total| progress.lock().unwrap().push((n, total)),
                    false
                )
                .unwrap(),
                b"exact"
            );
            assert_eq!(progress.into_inner().unwrap().last(), Some(&(5, Some(5))));
            task.join().unwrap();
        }
        #[test]
        fn environment_proxy_is_honoured_by_the_actual_socket_request() {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let task = thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut header = Vec::new();
                let mut byte = [0];
                while !header.ends_with(b"\r\n\r\n") {
                    stream.read_exact(&mut byte).unwrap();
                    header.push(byte[0]);
                }
                assert!(String::from_utf8(header)
                    .unwrap()
                    .starts_with("CONNECT never-resolve.invalid:80 HTTP/1.1"));
                stream
                    .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                    .unwrap();
                let mut header = Vec::new();
                while !header.ends_with(b"\r\n\r\n") {
                    stream.read_exact(&mut byte).unwrap();
                    header.push(byte[0]);
                }
                assert!(String::from_utf8(header)
                    .unwrap()
                    .starts_with("GET /fixture HTTP/1.1"));
                stream
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 7\r\n\r\nproxied")
                    .unwrap();
            });
            let status = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "files::fetch::review_tests::environment_proxy_child",
                    "--ignored",
                ])
                .env("KUDZU_PROXY_TEST", "1")
                .env("ALL_PROXY", format!("http://{address}"))
                .env_remove("all_proxy")
                .env_remove("HTTPS_PROXY")
                .env_remove("https_proxy")
                .env_remove("HTTP_PROXY")
                .env_remove("http_proxy")
                .env_remove("NO_PROXY")
                .env_remove("no_proxy")
                .status()
                .unwrap();
            assert!(status.success());
            task.join().unwrap();
        }
        #[test]
        #[ignore = "invoked only by the isolated loopback proxy parent"]
        fn environment_proxy_child() {
            assert_eq!(std::env::var("KUDZU_PROXY_TEST").unwrap(), "1");
            let req = FetchRequest::new(
                "http://never-resolve.invalid/fixture",
                Duration::from_secs(2),
                64,
            );
            assert_eq!(
                get_inner(&req, &|_, _| {}, false, true).unwrap(),
                b"proxied"
            );
        }
        #[test]
        fn query_only_redirect_replaces_query_on_the_same_resource() {
            assert_eq!(
                super::redirect("https://example.test/releases/latest.json?old=1", "?x=2").unwrap(),
                "https://example.test/releases/latest.json?x=2"
            );
        }
    }
}

#[cfg(test)]
mod fetch_tests {
    use super::fetch;
    use kudzu_app::{
        error::PortError,
        ports::{Fetch, FetchRequest},
    };
    use std::{
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        sync::{atomic::Ordering, Mutex},
        thread,
        time::{Duration, Instant},
    };

    // Every HTTP fixture binds loopback, disables proxies, and is joined by the test.
    fn server(
        handler: impl FnOnce(TcpStream) + Send + 'static,
    ) -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/fixture", listener.local_addr().unwrap());
        let task = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut header = Vec::new();
            let mut byte = [0];
            while !header.ends_with(b"\r\n\r\n") {
                if stream.read(&mut byte).unwrap_or(0) == 0 {
                    return;
                }
                header.push(byte[0]);
            }
            handler(stream);
        });
        (url, task)
    }
    fn request(url: String, cap: usize) -> FetchRequest {
        FetchRequest::new(url, Duration::from_secs(2), cap)
    }

    #[test]
    fn content_length_is_optional_and_progress_counts_bytes() {
        for known in [true, false] {
            let (url, server) = server(move |mut stream| {
                let header = if known {
                    "HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\n"
                } else {
                    "HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n"
                };
                stream.write_all(header.as_bytes()).unwrap();
                stream.write_all(b"hello").unwrap();
            });
            let progress = Mutex::new(Vec::new());
            assert_eq!(
                fetch::get(
                    &request(url, 5),
                    &|n, total| progress.lock().unwrap().push((n, total)),
                    false
                )
                .unwrap(),
                b"hello"
            );
            let progress = progress.into_inner().unwrap();
            assert_eq!(progress.first(), Some(&(0, known.then_some(5))));
            assert_eq!(progress.last(), Some(&(5, known.then_some(5))));
            server.join().unwrap();
        }
    }

    #[test]
    fn lying_lengths_are_bounded_and_content_encoded_archives_stay_byte_for_byte() {
        let (url, task) = server(|mut stream| {
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 999999999\r\n\r\nx");
        });
        assert_eq!(
            fetch::get(&request(url, 64), &|_, _| {}, false),
            Err(PortError::TooLarge)
        );
        task.join().unwrap();
        let (url, task) = server(|mut stream| {
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 20\r\n\r\nshort")
                .unwrap();
        });
        assert_eq!(
            fetch::get(&request(url, 64), &|_, _| {}, false),
            Err(PortError::InvalidData)
        );
        task.join().unwrap();
        // A conflicting, undersized Content-Length must not override a chunked body.
        let (url, task) = server(|mut stream| {
            let _=stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\nTransfer-Encoding: chunked\r\n\r\n100\r\n");
            let _ = stream.write_all(&[b'x'; 256]);
            let _ = stream.write_all(b"\r\n0\r\n\r\n");
        });
        assert!(fetch::get(&request(url, 64), &|_, _| {}, false).is_err());
        task.join().unwrap();
        let gzip = [
            31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 171, 168, 24, 5, 163, 96, 20, 140, 130, 81, 48, 10,
            70, 193, 72, 3, 0, 207, 52, 82, 111, 0, 8, 0, 0,
        ];
        let (url, task) = server(move |mut stream| {
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Encoding: gzip\r\n\r\n",
                gzip.len()
            )
            .unwrap();
            stream.write_all(&gzip).unwrap();
        });
        assert_eq!(
            fetch::get(&request(url, 64), &|_, _| {}, false).unwrap(),
            gzip
        );
        task.join().unwrap();
    }

    fn endless(delay: Duration) -> (String, thread::JoinHandle<()>) {
        server(move |mut stream| {
            if stream
                .write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n")
                .is_err()
            {
                return;
            }
            loop {
                if stream
                    .write_all(b"100\r\n")
                    .and_then(|_| stream.write_all(&[b'x'; 256]))
                    .and_then(|_| stream.write_all(b"\r\n"))
                    .is_err()
                {
                    break;
                }
                if !delay.is_zero() {
                    thread::sleep(delay);
                }
            }
        })
    }

    #[test]
    fn endless_body_is_bounded_by_size_and_a_global_deadline() {
        let (url, task) = endless(Duration::ZERO);
        assert_eq!(
            fetch::get(&request(url, 1024), &|n, _| assert!(n <= 1024), false),
            Err(PortError::TooLarge)
        );
        task.join().unwrap();
        let (url, task) = endless(Duration::from_millis(10));
        let mut req = request(url, usize::MAX);
        req.timeout = Duration::from_millis(75);
        let started = Instant::now();
        assert_eq!(
            fetch::get(&req, &|_, _| {}, false),
            Err(PortError::TimedOut)
        );
        assert!(started.elapsed() < Duration::from_secs(1));
        task.join().unwrap();
    }

    #[test]
    fn cancel_is_observed_before_io_and_while_reading() {
        let req = request("https://unused.invalid".into(), 64);
        req.cancel.store(true, Ordering::Release);
        assert_eq!(
            fetch::HttpFetch.get(&req, &|_, _| panic!("no IO")),
            Err(PortError::Cancelled)
        );
        let (url, task) = endless(Duration::from_millis(1));
        let req = request(url, usize::MAX);
        assert_eq!(
            fetch::get(
                &req,
                &|n, _| {
                    if n >= 256 {
                        req.cancel.store(true, Ordering::Release);
                    }
                },
                false
            ),
            Err(PortError::Cancelled)
        );
        task.join().unwrap();
    }

    #[test]
    fn redirects_cannot_downgrade_to_http_and_production_rejects_plain_http() {
        let (url, task) = server(|mut stream| {
            stream.write_all(b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/nope\r\nContent-Length: 0\r\n\r\n").unwrap();
        });
        assert_eq!(
            fetch::get(&request(url, 64), &|_, _| {}, false),
            Err(PortError::InvalidData)
        );
        task.join().unwrap();
        let req = request("http://127.0.0.1:1/nope".into(), 64);
        assert_eq!(
            fetch::HttpFetch.get(&req, &|_, _| {}),
            Err(PortError::Unavailable)
        );
    }
    #[test]
    fn cancel_interrupts_a_stalled_body_without_shortening_valid_slow_reads() {
        let (url, task) = server(|mut stream| {
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\n")
                .unwrap();
            let mut byte = [0];
            // The client must shut down the socket rather than wait for the request deadline.
            let _ = stream.read(&mut byte);
        });
        let req = request(url, 64);
        let cancel = req.cancel.clone();
        let (ready, wait) = std::sync::mpsc::sync_channel(1);
        let canceller = thread::spawn(move || {
            wait.recv().unwrap();
            thread::sleep(Duration::from_millis(50));
            cancel.store(true, Ordering::Release);
        });
        let started = Instant::now();
        assert_eq!(
            fetch::get(
                &req,
                &|_, _| {
                    let _ = ready.try_send(());
                },
                false
            ),
            Err(PortError::Cancelled)
        );
        assert!(started.elapsed() < Duration::from_secs(1));
        canceller.join().unwrap();
        task.join().unwrap();
        let (url, task) = server(|mut stream| {
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\n")
                .unwrap();
            thread::sleep(Duration::from_millis(200));
            stream.write_all(b"hello").unwrap();
        });
        assert_eq!(
            fetch::get(&request(url, 64), &|_, _| {}, false).unwrap(),
            b"hello"
        );
        task.join().unwrap();
    }
}
