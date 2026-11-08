//! `~/Library/Logs/<id>/Kudzu.log`, rotated at 2 MB keeping 3 old files, plus stderr in debug
//! builds. Same place and policy as tauri-plugin-log used, so support instructions still hold.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use log::{Level, LevelFilter, Log, Metadata, Record};
use parking_lot::Mutex;

const MAX_BYTES: u64 = 2_000_000;
const KEEP: usize = 3;

struct Logger {
    path: PathBuf,
    file: Mutex<Option<File>>,
    level: LevelFilter,
}

impl Logger {
    fn rotate(&self, file: &mut Option<File>) {
        let big = file.as_ref().and_then(|f| f.metadata().ok()).is_some_and(|m| m.len() > MAX_BYTES);
        if !big {
            return;
        }
        *file = None;
        for i in (1..KEEP).rev() {
            let _ = fs::rename(self.path.with_extension(format!("{i}.log")), self.path.with_extension(format!("{}.log", i + 1)));
        }
        let _ = fs::rename(&self.path, self.path.with_extension("1.log"));
        *file = OpenOptions::new().create(true).append(true).open(&self.path).ok();
    }
}

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        let noisy = ["winit", "wgpu", "naga", "zbus", "accesskit"].iter().any(|p| metadata.target().starts_with(p));
        metadata.level() <= self.level && (!noisy || metadata.level() <= Level::Warn)
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let line = format!(
            "[{}][{}][{}] {}\n",
            chrono::Local::now().format("%Y-%m-%d][%H:%M:%S"),
            record.target(),
            record.level(),
            record.args()
        );
        if cfg!(debug_assertions) {
            eprint!("{line}");
        }
        let mut file = self.file.lock();
        self.rotate(&mut file);
        if let Some(f) = file.as_mut() {
            let _ = f.write_all(line.as_bytes());
        }
    }

    fn flush(&self) {
        if let Some(f) = self.file.lock().as_mut() {
            let _ = f.flush();
        }
    }
}

pub fn init() {
    let dir = super::identity::log_dir();
    let _ = fs::create_dir_all(&dir);
    let path = dir.join("Kudzu.log");
    let file = OpenOptions::new().create(true).append(true).open(&path).ok();
    let level = if cfg!(debug_assertions) { LevelFilter::Debug } else { LevelFilter::Info };
    let logger = Box::leak(Box::new(Logger { path, file: Mutex::new(file), level }));
    if log::set_logger(logger).is_ok() {
        log::set_max_level(level);
    }
}
