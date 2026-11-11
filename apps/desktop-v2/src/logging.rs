use flexi_logger::{Cleanup, Criterion, Duplicate, FileSpec, Logger, LoggerHandle, Naming};
use std::{path::Path, sync::Mutex};

static LOGGER: Mutex<Option<LoggerHandle>> = Mutex::new(None);

fn logger(path: &Path) -> Logger {
    Logger::try_with_str(if cfg!(debug_assertions) {
        "debug,rune=warn,rune_staging=warn,rune_kit=warn,winit=warn,wgpu=warn,wgpu_core=warn,wgpu_hal=warn,wgpu_types=warn"
    } else {
        "info"
    })
    .expect("valid log level")
    .log_to_file(
        FileSpec::default()
            .directory(path)
            .basename("Kudzu")
            .suppress_timestamp(),
    )
    .duplicate_to_stderr(Duplicate::All)
    .append()
    .create_symlink(path.join("Kudzu.log"))
    .cleanup_in_background_thread(false)
    .rotate(
        Criterion::Size(2_000_000),
        Naming::Numbers,
        Cleanup::KeepLogFiles(2),
    )
}

pub fn init(path: &Path) -> Result<(), flexi_logger::FlexiLoggerError> {
    let mut handle = LOGGER.lock().expect("logger initialization lock poisoned");
    if handle.is_none() {
        *handle = Some(logger(path).start()?);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use log::{Level, Log, Record};
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn initialization_is_idempotent_under_concurrency() {
        let path = std::env::temp_dir().join(format!(
            "kudzu-log-init-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::thread::scope(|scope| {
            for _ in 0..8 {
                scope.spawn(|| init(&path).unwrap());
            }
        });
        log::info!("initialization test");
        log::logger().flush();
        assert!(path.join("Kudzu.log").exists());
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn noisy_dependency_debug_messages_are_filtered() {
        use log::Metadata;
        let path = std::env::temp_dir().join(format!(
            "kudzu-log-filter-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let (logger, handle) = logger(&path)
            .duplicate_to_stderr(Duplicate::None)
            .build()
            .unwrap();
        for target in [
            "rune::widget",
            "winit::event",
            "wgpu_core::device",
            "wgpu_hal::metal",
        ] {
            assert!(
                !logger.enabled(
                    &Metadata::builder()
                        .level(Level::Debug)
                        .target(target)
                        .build()
                ),
                "{target}"
            );
        }
        if cfg!(debug_assertions) {
            assert!(logger.enabled(
                &Metadata::builder()
                    .level(Level::Debug)
                    .target("kudzu")
                    .build()
            ));
        }
        drop(handle);
        drop(logger);
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn rotates_at_two_megabytes_and_retains_three_files() {
        let path = std::env::temp_dir().join(format!(
            "kudzu-log-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let (logger, handle) = logger(&path)
            .duplicate_to_stderr(Duplicate::None)
            .build()
            .unwrap();
        let line = "x".repeat(100_000);
        for _ in 0..90 {
            logger.log(
                &Record::builder()
                    .args(format_args!("{}", line))
                    .level(Level::Info)
                    .target("rotation-test")
                    .build(),
            );
        }
        logger.flush();
        handle.flush();
        let mut files: Vec<_> = fs::read_dir(&path)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| {
                p.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("Kudzu_r")
            })
            .collect();
        files.sort();
        assert_eq!(
            files.len(),
            3,
            "current file plus two retained archives: {files:?}"
        );
        assert!(path.join("Kudzu.log").exists());
        for file in files
            .iter()
            .filter(|p| !p.to_string_lossy().contains("CURRENT"))
        {
            let size = fs::metadata(file).unwrap().len();
            assert!(
                (2_000_000..=2_100_500).contains(&size),
                "rotation size {size}"
            );
        }
        drop(handle);
        drop(logger);
        fs::remove_dir_all(path).unwrap();
    }
}
