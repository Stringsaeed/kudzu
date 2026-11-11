use flexi_logger::{Cleanup, Criterion, Duplicate, FileSpec, Logger, LoggerHandle, Naming};
use std::path::Path;

fn logger(path: &Path) -> Logger {
    Logger::try_with_str(if cfg!(debug_assertions) {
        "debug"
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

pub fn init(path: &Path) -> Result<LoggerHandle, flexi_logger::FlexiLoggerError> {
    logger(path).start()
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
