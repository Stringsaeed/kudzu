//! Environment overrides plus macOS per-URL proxy settings, exceptions and PAC.
use kudzu_app::{error::PortError, ports::FetchRequest};
use std::time::Instant;

pub(super) fn resolve(
    request: &FetchRequest,
    url: &str,
    started: Instant,
) -> Result<Option<ureq::Proxy>, PortError> {
    if let Some(proxy) = ureq::Proxy::try_from_env() {
        return Ok(Some(proxy));
    }
    #[cfg(target_os = "macos")]
    {
        macos::system(request, url, started)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (request, url, started);
        Ok(None)
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use core_foundation::{
        array::{CFArray, CFArrayRef},
        base::{CFType, TCFType},
        dictionary::{CFDictionary, CFDictionaryRef},
        number::CFNumber,
        runloop::{
            kCFRunLoopDefaultMode, CFRunLoop, CFRunLoopSource, CFRunLoopSourceInvalidate,
            CFRunLoopSourceRef,
        },
        string::{CFString, CFStringRef},
        url::{CFURLCreateWithString, CFURLRef, CFURL},
    };
    use std::{ffi::c_void, sync::atomic::Ordering, time::Duration};
    type Proxies = CFArray<CFDictionary<CFString, CFType>>;
    type Callback = extern "C" fn(*mut c_void, CFArrayRef, *const c_void);
    #[repr(C)]
    struct Context {
        version: isize,
        info: *mut c_void,
        retain: Option<extern "C" fn(*const c_void) -> *const c_void>,
        release: Option<extern "C" fn(*const c_void)>,
        describe: Option<extern "C" fn(*const c_void) -> CFStringRef>,
    }
    #[link(name = "CFNetwork", kind = "framework")]
    unsafe extern "C" {
        fn CFNetworkCopySystemProxySettings() -> CFDictionaryRef;
        fn CFNetworkCopyProxiesForURL(url: CFURLRef, settings: CFDictionaryRef) -> CFArrayRef;
        fn CFNetworkExecuteProxyAutoConfigurationScript(
            script: CFStringRef,
            url: CFURLRef,
            callback: Callback,
            context: *mut Context,
        ) -> CFRunLoopSourceRef;
        static kCFProxyTypeKey: CFStringRef;
        static kCFProxyTypeNone: CFStringRef;
        static kCFProxyTypeHTTP: CFStringRef;
        static kCFProxyTypeHTTPS: CFStringRef;
        static kCFProxyTypeSOCKS: CFStringRef;
        static kCFProxyTypeAutoConfigurationURL: CFStringRef;
        static kCFProxyTypeAutoConfigurationJavaScript: CFStringRef;
        static kCFProxyHostNameKey: CFStringRef;
        static kCFProxyPortNumberKey: CFStringRef;
        static kCFProxyUsernameKey: CFStringRef;
        static kCFProxyPasswordKey: CFStringRef;
        static kCFProxyAutoConfigurationURLKey: CFStringRef;
        static kCFProxyAutoConfigurationJavaScriptKey: CFStringRef;
    }
    fn value(dict: &CFDictionary<CFString, CFType>, key: CFStringRef) -> Option<CFType> {
        dict.find(&unsafe { CFString::wrap_under_get_rule(key) })
            .map(|v| (*v).clone())
    }
    fn url(value: &str) -> Result<CFURL, PortError> {
        let raw = unsafe {
            CFURLCreateWithString(
                std::ptr::null(),
                CFString::new(value).as_concrete_TypeRef(),
                std::ptr::null(),
            )
        };
        if raw.is_null() {
            Err(PortError::InvalidData)
        } else {
            Ok(unsafe { CFURL::wrap_under_create_rule(raw) })
        }
    }
    pub(super) fn system(
        request: &FetchRequest,
        target: &str,
        started: Instant,
    ) -> Result<Option<ureq::Proxy>, PortError> {
        let raw = unsafe { CFNetworkCopySystemProxySettings() };
        if raw.is_null() {
            return Err(PortError::Unavailable);
        }
        let settings = unsafe { CFDictionary::<CFString, CFType>::wrap_under_create_rule(raw) };
        configured(request, &url(target)?, &settings, started)
    }
    fn configured(
        request: &FetchRequest,
        url: &CFURL,
        settings: &CFDictionary<CFString, CFType>,
        started: Instant,
    ) -> Result<Option<ureq::Proxy>, PortError> {
        let raw = unsafe {
            CFNetworkCopyProxiesForURL(url.as_concrete_TypeRef(), settings.as_concrete_TypeRef())
        };
        if raw.is_null() {
            return Err(PortError::Unavailable);
        }
        select(
            request,
            url,
            &unsafe { Proxies::wrap_under_create_rule(raw) },
            started,
            0,
        )
    }
    fn select(
        request: &FetchRequest,
        url: &CFURL,
        list: &Proxies,
        started: Instant,
        depth: usize,
    ) -> Result<Option<ureq::Proxy>, PortError> {
        if depth > 4 {
            return Err(PortError::InvalidData);
        }
        let dict = list.get(0).ok_or(PortError::Unavailable)?;
        let kind = value(&dict, unsafe { kCFProxyTypeKey })
            .and_then(|v| v.downcast::<CFString>())
            .ok_or(PortError::InvalidData)?;
        let is = |key| kind == unsafe { CFString::wrap_under_get_rule(key) };
        if is(unsafe { kCFProxyTypeNone }) {
            return Ok(None);
        }
        if is(unsafe { kCFProxyTypeAutoConfigurationURL })
            || is(unsafe { kCFProxyTypeAutoConfigurationJavaScript })
        {
            let list = pac(request, url, &dict, started)?;
            return select(request, url, &list, started, depth + 1);
        }
        let protocol = if is(unsafe { kCFProxyTypeHTTP }) || is(unsafe { kCFProxyTypeHTTPS }) {
            ureq::ProxyProtocol::Http
        } else if is(unsafe { kCFProxyTypeSOCKS }) {
            ureq::ProxyProtocol::Socks5
        } else {
            return Err(PortError::Unavailable);
        };
        let host = value(&dict, unsafe { kCFProxyHostNameKey })
            .and_then(|v| v.downcast::<CFString>())
            .ok_or(PortError::InvalidData)?
            .to_string();
        let port = value(&dict, unsafe { kCFProxyPortNumberKey })
            .and_then(|v| v.downcast::<CFNumber>())
            .and_then(|n| n.to_i64())
            .and_then(|n| u16::try_from(n).ok())
            .filter(|n| *n > 0)
            .ok_or(PortError::InvalidData)?;
        let mut proxy = ureq::Proxy::builder(protocol).host(&host).port(port);
        if let Some(username) =
            value(&dict, unsafe { kCFProxyUsernameKey }).and_then(|v| v.downcast::<CFString>())
        {
            proxy = proxy.username(&username.to_string());
        }
        if let Some(password) =
            value(&dict, unsafe { kCFProxyPasswordKey }).and_then(|v| v.downcast::<CFString>())
        {
            proxy = proxy.password(&password.to_string());
        }
        proxy.build().map(Some).map_err(|_| PortError::InvalidData)
    }
    struct Pending {
        result: Option<Result<Proxies, PortError>>,
    }
    extern "C" fn completed(info: *mut c_void, list: CFArrayRef, error: *const c_void) {
        // CFNetwork delivers this callback on the run loop on which we schedule its source.
        let pending = unsafe { &mut *info.cast::<Pending>() };
        pending.result = Some(if !error.is_null() || list.is_null() {
            Err(PortError::Unavailable)
        } else {
            Ok(unsafe { Proxies::wrap_under_get_rule(list) })
        });
    }
    struct Source(CFRunLoopSource);
    impl Drop for Source {
        fn drop(&mut self) {
            unsafe {
                CFRunLoopSourceInvalidate(self.0.as_concrete_TypeRef());
            }
        }
    }
    fn pac(
        request: &FetchRequest,
        url: &CFURL,
        dict: &CFDictionary<CFString, CFType>,
        started: Instant,
    ) -> Result<Proxies, PortError> {
        if request.cancel.load(Ordering::Acquire) {
            return Err(PortError::Cancelled);
        }
        if started.elapsed() >= request.timeout {
            return Err(PortError::TimedOut);
        }
        let mut pending = Pending { result: None };
        let mut context = Context {
            version: 0,
            info: (&mut pending as *mut Pending).cast(),
            retain: None,
            release: None,
            describe: None,
        };
        let script = if let Some(pac) = value(dict, unsafe { kCFProxyAutoConfigurationURLKey })
            .and_then(|v| v.downcast::<CFURL>())
        {
            // Bootstrap directly: resolving this download through PAC would recurse.
            // The app's bounded/cancellable transport owns all network I/O; CFNetwork
            // evaluates scripts without using its opaque PAC URL download path.
            let address = pac.get_string().to_string();
            let bytes = if address
                .split_once(':')
                .is_some_and(|(scheme, _)| scheme.eq_ignore_ascii_case("file"))
            {
                let path = pac.to_path().ok_or(PortError::InvalidData)?;
                use std::io::Read;
                let mut bytes = Vec::new();
                std::fs::File::open(path)
                    .map_err(|e| PortError::Io(e.into()))?
                    .take(1024 * 1024 + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|e| PortError::Io(e.into()))?;
                if bytes.len() > 1024 * 1024 {
                    return Err(PortError::TooLarge);
                }
                bytes
            } else {
                let bootstrap = FetchRequest {
                    url: address,
                    timeout: request
                        .timeout
                        .checked_sub(started.elapsed())
                        .ok_or(PortError::TimedOut)?,
                    max_bytes: 1024 * 1024,
                    cancel: request.cancel.clone(),
                };
                super::super::get_inner(&bootstrap, &|_, _| {}, false, false)?
            };
            CFString::new(&String::from_utf8(bytes).map_err(|_| PortError::InvalidData)?)
        } else {
            value(dict, unsafe { kCFProxyAutoConfigurationJavaScriptKey })
                .and_then(|v| v.downcast::<CFString>())
                .ok_or(PortError::InvalidData)?
        };
        if request.cancel.load(Ordering::Acquire) {
            return Err(PortError::Cancelled);
        }
        if started.elapsed() >= request.timeout {
            return Err(PortError::TimedOut);
        }
        let raw = unsafe {
            CFNetworkExecuteProxyAutoConfigurationScript(
                script.as_concrete_TypeRef(),
                url.as_concrete_TypeRef(),
                completed,
                &mut context,
            )
        };
        if raw.is_null() {
            return Err(PortError::Unavailable);
        }
        let source = Source(unsafe { CFRunLoopSource::wrap_under_create_rule(raw) });
        let mode = unsafe { kCFRunLoopDefaultMode };
        CFRunLoop::get_current().add_source(&source.0, mode);
        loop {
            if request.cancel.load(Ordering::Acquire) {
                return Err(PortError::Cancelled);
            }
            let remaining = request
                .timeout
                .checked_sub(started.elapsed())
                .ok_or(PortError::TimedOut)?;
            if let Some(result) = pending.result.take() {
                return result;
            }
            CFRunLoop::run_in_mode(mode, remaining.min(Duration::from_millis(20)), true);
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        fn request() -> FetchRequest {
            FetchRequest::new(
                "https://fixture.invalid/releases/latest.json",
                Duration::from_secs(2),
                64,
            )
        }
        #[test]
        fn system_dictionary_proxy_and_exceptions_are_resolved_per_url() {
            let settings = CFDictionary::from_CFType_pairs(&[
                (
                    CFString::new("HTTPSEnable"),
                    CFNumber::from(1_i32).as_CFType(),
                ),
                (
                    CFString::new("HTTPSProxy"),
                    CFString::new("127.0.0.1").as_CFType(),
                ),
                (
                    CFString::new("HTTPSPort"),
                    CFNumber::from(8123_i32).as_CFType(),
                ),
                (
                    CFString::new("ExceptionsList"),
                    CFArray::from_CFTypes(&[CFString::new("bypass.invalid")]).as_CFType(),
                ),
            ]);
            let req = request();
            let proxy = configured(&req, &url(&req.url).unwrap(), &settings, Instant::now())
                .unwrap()
                .unwrap();
            assert_eq!((proxy.host(), proxy.port()), ("127.0.0.1", 8123));
            assert!(configured(
                &req,
                &url("https://bypass.invalid/fixture").unwrap(),
                &settings,
                Instant::now()
            )
            .unwrap()
            .is_none());
        }
        #[test]
        fn pac_script_is_executed_on_a_bounded_cancellable_run_loop() {
            let req = request();
            let script = CFString::new(
                "function FindProxyForURL(url, host) { return 'PROXY 127.0.0.1:8124'; }",
            );
            let settings = CFDictionary::from_CFType_pairs(&[
                (
                    CFString::new("ProxyAutoConfigJavaScript"),
                    script.as_CFType(),
                ),
                (
                    CFString::new("ProxyAutoConfigEnable"),
                    CFNumber::from(1_i32).as_CFType(),
                ),
            ]);
            let proxy = configured(&req, &url(&req.url).unwrap(), &settings, Instant::now())
                .unwrap()
                .unwrap();
            assert_eq!((proxy.host(), proxy.port()), ("127.0.0.1", 8124));
            req.cancel.store(true, Ordering::Release);
            assert!(matches!(
                configured(&req, &url(&req.url).unwrap(), &settings, Instant::now()),
                Err(PortError::Cancelled)
            ));
        }
        #[test]
        fn pac_url_is_downloaded_from_a_local_fixture_and_obeys_cancel_and_deadline() {
            use std::{
                io::{Read, Write},
                net::TcpListener,
                thread,
            };
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let task = thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut request = Vec::new();
                let mut byte = [0];
                while !request.ends_with(b"\r\n\r\n") {
                    stream.read_exact(&mut byte).unwrap();
                    request.push(byte[0]);
                }
                let script =
                    "function FindProxyForURL(url, host) { return 'PROXY 127.0.0.1:8125'; }";
                write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/x-ns-proxy-autoconfig\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",script.len(),script).unwrap();
            });
            let settings = CFDictionary::from_CFType_pairs(&[
                (
                    CFString::new("ProxyAutoConfigEnable"),
                    CFNumber::from(1_i32).as_CFType(),
                ),
                (
                    CFString::new("ProxyAutoConfigURLString"),
                    CFString::new(&format!("http://{address}/fixture.pac")).as_CFType(),
                ),
            ]);
            let mut req = request();
            let target = url(&req.url).unwrap();
            let proxy = configured(&req, &target, &settings, Instant::now())
                .unwrap()
                .unwrap();
            assert_eq!((proxy.host(), proxy.port()), ("127.0.0.1", 8125));
            task.join().unwrap();
            req.cancel.store(true, Ordering::Release);
            assert!(matches!(
                configured(&req, &target, &settings, Instant::now()),
                Err(PortError::Cancelled)
            ));
            req.cancel.store(false, Ordering::Release);
            req.timeout = Duration::ZERO;
            assert!(matches!(
                configured(&req, &target, &settings, Instant::now()),
                Err(PortError::TimedOut)
            ));
        }
        #[test]
        fn file_pac_urls_are_explicitly_file_scoped_and_size_bounded() {
            let dir =
                crate::update_install::location::Scratch::beside(&std::env::temp_dir()).unwrap();
            let path = dir.0.join("fixture.pac");
            std::fs::write(
                &path,
                "function FindProxyForURL(url, host) { return 'PROXY 127.0.0.1:8126'; }",
            )
            .unwrap();
            let dict = CFDictionary::from_CFType_pairs(&[(
                unsafe { CFString::wrap_under_get_rule(kCFProxyAutoConfigurationURLKey) },
                CFURL::from_path(&path, false).unwrap().as_CFType(),
            )]);
            let req = request();
            let target = url(&req.url).unwrap();
            let list = pac(&req, &target, &dict, Instant::now()).unwrap();
            assert_eq!(
                select(&req, &target, &list, Instant::now(), 0)
                    .unwrap()
                    .unwrap()
                    .port(),
                8126
            );
            std::fs::OpenOptions::new()
                .write(true)
                .open(&path)
                .unwrap()
                .set_len(1024 * 1024 + 1)
                .unwrap();
            assert!(matches!(
                pac(&req, &target, &dict, Instant::now()),
                Err(PortError::TooLarge)
            ));
        }
    }
}
