use crate::bundle::{self, Variant};
use std::{error::Error, process::Command};

pub fn run() -> Result<(), Box<dyn Error>> {
    let app = bundle::build(Variant::Dev, false)?;
    let status = Command::new("open").arg(&app).status()?;
    if !status.success() {
        return Err(format!("open failed: {status}").into());
    }
    println!("Opened {}", app.display());
    Ok(())
}
