use super::{engine, requests, CloudBatch, Session};
use objc2::{
    define_class, msg_send,
    rc::{autoreleasepool, Retained},
    runtime::{NSObject, NSObjectProtocol},
    AllocAnyThread, DefinedClass,
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "KudzuCloudAutoreleaseProbe"]
    #[ivars = Arc<AtomicUsize>]
    struct Probe;
    unsafe impl NSObjectProtocol for Probe {}
);

impl Probe {
    fn autorelease(dropped: Arc<AtomicUsize>) {
        let this = Self::alloc().set_ivars(dropped);
        let object: Retained<Self> = unsafe { msg_send![super(this), init] };
        let _ = Retained::autorelease_ptr(object);
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        self.ivars().fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn worker_requests_drain_autoreleased_objects_before_returning() {
    autoreleasepool(|_| {
        let dropped = Arc::new(AtomicUsize::new(0));
        let sent = dropped.clone();
        let fetched = dropped.clone();
        let engine = engine::Engine {
            on_send: Some(Box::new(move |_| Probe::autorelease(sent.clone()))),
            on_fetch: Some(Box::new(move |completion| {
                Probe::autorelease(fetched.clone());
                completion.unwrap().call((std::ptr::null_mut(),));
            })),
            ..Default::default()
        };
        let session = Arc::new(Session::new(1, |_, _| {}));
        requests::send(&engine, &session, CloudBatch::default());
        assert_eq!(
            dropped.load(Ordering::SeqCst),
            1,
            "send must drain its own pool, not wait for the worker to exit"
        );
        requests::fetch(&engine, &session);
        assert_eq!(
            dropped.load(Ordering::SeqCst),
            2,
            "fetch must drain its own pool"
        );
    });
}
