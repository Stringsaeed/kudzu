use crate::bundle::{self, Variant};
use std::{error::Error, process::Command};

pub fn run() -> Result<(), Box<dyn Error>> {
    let app = bundle::build(Variant::Dev, false)?;
    if let Some(identity) = std::env::var_os("KUDZU_DEV_SIGN").filter(|s| !s.is_empty()) {
        let status = Command::new("codesign")
            .args(["--force", "--sign"])
            .arg(identity)
            .arg(&app)
            .status()?;
        if !status.success() {
            return Err(format!("codesign failed: {status}").into());
        }
    }
    let status = Command::new("open").arg(&app).status()?;
    if !status.success() {
        return Err(format!("open failed: {status}").into());
    }
    println!("Opened {}", app.display());
    Ok(())
}
