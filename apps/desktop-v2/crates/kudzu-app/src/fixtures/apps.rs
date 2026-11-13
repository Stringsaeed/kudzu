//! Catalogue and focus fixture, with no NSWorkspace or AX access.
use crate::{
    error::PortError,
    ports::{AppCatalog, FileIcons, TargetApp},
};
use kudzu_core::system::{AppInfo, Rect};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};

/// A one-way gate a test opens: workers wait on it, so a test can hold an operation in the
/// middle without sleeping.
#[derive(Default)]
pub struct Latch {
    open: Mutex<bool>,
    changed: Condvar,
}
impl Latch {
    pub fn set(&self) {
        *self.open.lock().unwrap() = true;
        self.changed.notify_all();
    }
    /// Waits for [`Latch::set`]. Panics after 10 s, so a test whose worker never gets there
    /// fails instead of hanging.
    pub fn wait(&self) {
        let open = self.open.lock().unwrap();
        let (open, _) = self
            .changed
            .wait_timeout_while(open, Duration::from_secs(10), |open| !*open)
            .unwrap();
        assert!(*open, "the latch was not opened within 10 s");
    }
}
#[derive(Default)]
pub struct FakeApps {
    pub installed: Mutex<Vec<AppInfo>>,
    pub icons: Mutex<BTreeMap<String, Vec<u8>>>,
    pub frontmost: Mutex<Option<AppInfo>>,
    pub selection: Mutex<Option<String>>,
    pub selection_reads: Mutex<usize>,
    pub activations: Mutex<Vec<AppInfo>>,
    pub caret: Mutex<Option<Rect>>,
    pub failure: Mutex<Option<PortError>>,
    pub installed_calls: Mutex<usize>,
    pub icon_calls: Mutex<Vec<(String, u32)>>,
    /// While set, `activate` waits for this latch after setting `activation_entered`.
    pub activation_hold: Mutex<Option<Arc<Latch>>>,
    pub activation_entered: Latch,
    /// Answers for the next `frontmost` calls, oldest first; then the current app again.
    pub frontmost_script: Mutex<VecDeque<Option<AppInfo>>>,
}
impl FakeApps {
    fn check(&self) -> Result<(), PortError> {
        self.failure.lock().unwrap().map_or(Ok(()), Err)
    }
}
impl AppCatalog for FakeApps {
    fn installed(&self) -> Result<Vec<AppInfo>, PortError> {
        self.check()?;
        *self.installed_calls.lock().unwrap() += 1;
        Ok(self.installed.lock().unwrap().clone())
    }
    fn icon(&self, id: &str, size: u32) -> Result<Option<Vec<u8>>, PortError> {
        self.check()?;
        self.icon_calls.lock().unwrap().push((id.into(), size));
        Ok(self.icons.lock().unwrap().get(id).cloned())
    }
}
impl TargetApp for FakeApps {
    fn frontmost(&self) -> Result<Option<AppInfo>, PortError> {
        self.check()?;
        if let Some(answer) = self.frontmost_script.lock().unwrap().pop_front() {
            return Ok(answer);
        }
        Ok(self.frontmost.lock().unwrap().clone())
    }
    fn activate(&self, app: &AppInfo) -> Result<(), PortError> {
        self.check()?;
        self.activation_entered.set();
        let hold = self.activation_hold.lock().unwrap().clone();
        if let Some(hold) = hold {
            hold.wait();
        }
        self.activations.lock().unwrap().push(app.clone());
        *self.frontmost.lock().unwrap() = Some(app.clone());
        Ok(())
    }
    fn selection(&self) -> Result<Option<String>, PortError> {
        *self.selection_reads.lock().unwrap() += 1;
        self.check()?;
        Ok(self.selection.lock().unwrap().clone())
    }
    fn caret(&self) -> Result<Option<Rect>, PortError> {
        self.check()?;
        Ok(*self.caret.lock().unwrap())
    }
}

impl FileIcons for FakeApps {
    fn file_icon(&self, path: &str, px: u32) -> Result<Option<Vec<u8>>, PortError> {
        self.check()?;
        self.icon_calls.lock().unwrap().push((path.to_string(), px));
        Ok(self.icons.lock().unwrap().get(path).cloned())
    }
}
