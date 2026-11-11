//! Paths are supplied by composition. This module never discovers or creates directories.
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    pub data: PathBuf,
    pub logs: PathBuf,
}
impl Paths {
    pub fn library(&self) -> PathBuf {
        self.data.join("library.json")
    }
    pub fn settings(&self) -> PathBuf {
        self.data.join("settings.json")
    }
    pub fn form_memory(&self) -> PathBuf {
        self.data.join("form-memory.json")
    }
    pub fn ui_state(&self) -> PathBuf {
        self.data.join("ui-state.json")
    }
    pub fn backups(&self) -> PathBuf {
        self.data.join("backups")
    }
    pub fn clipboard(&self) -> PathBuf {
        self.data.join("clipboard")
    }
    pub fn sync(&self) -> PathBuf {
        self.data.join("sync")
    }
    pub fn device_id(&self) -> PathBuf {
        self.data.join("device-id")
    }
    pub fn agent_socket(&self) -> PathBuf {
        self.data.join("agent.sock")
    }
    pub fn permission_restart(&self) -> PathBuf {
        self.data.join("permission-setup-pending")
    }
}
