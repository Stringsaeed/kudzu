use std::{
    fs::OpenOptions,
    io::Write,
    time::{SystemTime, UNIX_EPOCH},
};

pub fn mark(name: &str) {
    let Some(path) = std::env::var_os("KUDZU_TIMINGS") else {
        return;
    };
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        if let Ok(now) = SystemTime::now().duration_since(UNIX_EPOCH) {
            let _ = writeln!(file, "{} {}", name, now.as_nanos());
        }
    }
}
