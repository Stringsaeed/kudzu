use crate::release::Result;
use base64::{engine::general_purpose::STANDARD, Engine};
use std::{
    fs,
    io::Cursor,
    path::{Path, PathBuf},
    process::Command,
};

pub fn archive_signature(
    bytes: &[u8],
    wrapped: &str,
    password: &str,
    version: &str,
    timestamp: i64,
) -> Result<String> {
    let decoded = STANDARD
        .decode(wrapped.trim())
        .map_err(|_| "invalid wrapped updater key")?;
    let text = std::str::from_utf8(&decoded).map_err(|_| "invalid updater key text")?;
    let key = minisign::SecretKeyBox::from_string(text)
        .and_then(|b| b.into_secret_key(Some(password.to_owned())))
        .map_err(|_| "cannot unlock updater key")?;
    let comment = format!("timestamp:{timestamp}\tfile:Kudzu_{version}_universal.app.tar.gz");
    let sig = minisign::sign(None, &key, Cursor::new(bytes), Some(&comment), None)
        .map_err(|_| "updater signing failed")?;
    Ok(STANDARD.encode(sig.to_string()))
}

// Secret-bearing commands never inherit output or appear in errors.
fn security(args: &[&str]) -> Result<String> {
    let output = Command::new("security").args(args).output()?;
    if !output.status.success() {
        return Err("release keychain operation failed (details suppressed)".into());
    }
    Ok(String::from_utf8(output.stdout)?)
}
pub struct Keychain {
    path: PathBuf,
    original: Vec<String>,
    run: fn(&[&str]) -> Result<String>,
}
impl Keychain {
    pub fn import(dir: &Path) -> Result<Option<Self>> {
        let Ok(certificate) = std::env::var("APPLE_CERTIFICATE") else {
            return Ok(None);
        };
        let password = std::env::var("APPLE_CERTIFICATE_PASSWORD")
            .map_err(|_| "APPLE_CERTIFICATE_PASSWORD required")?;
        Self::create(dir, &certificate, &password, security).map(Some)
    }
    fn create(
        dir: &Path,
        certificate: &str,
        cert_password: &str,
        run: fn(&[&str]) -> Result<String>,
    ) -> Result<Self> {
        let original = run(&["list-keychains", "-d", "user"])?
            .lines()
            .map(|s| s.trim().trim_matches('"').to_owned())
            .collect();
        let guard = Self {
            path: dir.join("release.keychain-db"),
            original,
            run,
        };
        fs::write(
            dir.join("keychain-search-list.json"),
            serde_json::to_vec(&guard.original)?,
        )?;
        let path = guard.path.to_str().ok_or("invalid keychain path")?;
        let password = Command::new("openssl")
            .args(["rand", "-hex", "32"])
            .output()?;
        if !password.status.success() {
            return Err("keychain password generation failed".into());
        }
        let password = String::from_utf8(password.stdout)?;
        run(&["create-keychain", "-p", password.trim(), path])?;
        run(&["set-keychain-settings", "-lut", "21600", path])?;
        run(&["unlock-keychain", "-p", password.trim(), path])?;
        let p12 = dir.join("certificate.p12");
        fs::write(
            &p12,
            STANDARD
                .decode(certificate.trim())
                .map_err(|_| "invalid certificate encoding")?,
        )?;
        run(&[
            "import",
            p12.to_str().unwrap(),
            "-k",
            path,
            "-P",
            cert_password,
            "-T",
            "/usr/bin/codesign",
            "-T",
            "/usr/bin/security",
        ])?;
        fs::remove_file(p12)?;
        run(&[
            "set-key-partition-list",
            "-S",
            "apple-tool:,apple:,codesign:",
            "-s",
            "-k",
            password.trim(),
            path,
        ])?;
        let mut list = vec!["list-keychains", "-d", "user", "-s", path];
        list.extend(guard.original.iter().map(String::as_str));
        run(&list)?;
        Ok(guard)
    }
    pub fn close(&self) -> Result<()> {
        let mut list = vec!["list-keychains", "-d", "user", "-s"];
        list.extend(self.original.iter().map(String::as_str));
        let restored = (self.run)(&list);
        let deleted = if self.path.exists() {
            (self.run)(&["delete-keychain", self.path.to_str().unwrap()])
        } else {
            Ok(String::new())
        };
        restored?;
        deleted?;
        if self.path.exists() {
            return Err("release keychain still exists".into());
        }
        let journal = self
            .path
            .parent()
            .unwrap()
            .join("keychain-search-list.json");
        if journal.exists() {
            fs::remove_file(journal)?;
        }
        Ok(())
    }
}

pub fn cleanup() -> Result<()> {
    let target = crate::release::root().join("target");
    if !target.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(target)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir()
            || !entry
                .file_name()
                .to_string_lossy()
                .starts_with("release-private-")
        {
            continue;
        }
        let dir = entry.path();
        let journal = dir.join("keychain-search-list.json");
        if journal.exists() {
            let guard = Keychain {
                path: dir.join("release.keychain-db"),
                original: serde_json::from_slice(&fs::read(journal)?)?,
                run: security,
            };
            guard.close()?;
        }
        if dir.join("release.keychain-db").exists() {
            return Err("keychain exists without cleanup journal".into());
        }
        fs::remove_dir_all(dir)?;
    }
    println!("temporary release keychains removed");
    Ok(())
}
impl Drop for Keychain {
    fn drop(&mut self) {
        if self
            .path
            .parent()
            .unwrap()
            .join("keychain-search-list.json")
            .exists()
            && self.close().is_err()
        {
            eprintln!("ERROR: release keychain cleanup failed");
        }
    }
}

#[cfg(test)]
#[allow(dead_code)]
#[path = "../../crates/kudzu-app/src/updates/error.rs"]
mod error;
#[cfg(test)]
#[allow(dead_code)]
#[path = "../../crates/kudzu-app/src/updates/verify.rs"]
mod verify;
#[cfg(test)]
mod tests {
    use super::*;
    thread_local! {
        static CALLS: std::cell::RefCell<Vec<Vec<String>>> = const { std::cell::RefCell::new(Vec::new()) };
        static FAIL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }
    fn fake_security(args: &[&str]) -> Result<String> {
        CALLS.with(|calls| {
            calls
                .borrow_mut()
                .push(args.iter().map(|s| s.to_string()).collect())
        });
        match args[0] {
            "list-keychains" if args.len() == 3 => {
                return Ok("  \"/original/login.keychain-db\"\n".into())
            }
            "create-keychain" => fs::write(args.last().unwrap(), "fake keychain")?,
            "delete-keychain" => fs::remove_file(args[1])?,
            "import" if FAIL.with(|f| f.get()) => return Err("injected import failure".into()),
            _ => {}
        }
        Ok(String::new())
    }
    #[test]
    fn keychain_restores_search_list_on_success_and_import_failure() {
        let dir = crate::release::root()
            .join("target")
            .join(format!("s70-keychain-test-{}", std::process::id()));
        fs::create_dir(&dir).unwrap();
        for fail in [false, true] {
            FAIL.with(|f| f.set(fail));
            CALLS.with(|c| c.borrow_mut().clear());
            let result = Keychain::create(
                &dir,
                &STANDARD.encode("throwaway p12"),
                "throwaway",
                fake_security,
            );
            if fail {
                assert!(result.is_err());
            } else {
                result.unwrap().close().unwrap();
            }
            assert!(!dir.join("release.keychain-db").exists());
            assert!(!dir.join("keychain-search-list.json").exists());
            CALLS.with(|calls| {
                let calls = calls.borrow();
                assert_eq!(
                    calls[calls.len() - 2],
                    [
                        "list-keychains",
                        "-d",
                        "user",
                        "-s",
                        "/original/login.keychain-db"
                    ]
                );
                assert_eq!(calls.last().unwrap()[0], "delete-keychain");
            });
        }
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn wrapped_encrypted_key_feed_verifies_with_s31() {
        let pair = minisign::KeyPair::generate_encrypted_keypair(Some("throwaway".into())).unwrap();
        let key = STANDARD.encode(pair.sk.to_box(None).unwrap().to_string());
        let bytes = b"throwaway archive payload";
        let signature = archive_signature(bytes, &key, "throwaway", "9.9.9-beta.2", 123).unwrap();
        let text = String::from_utf8(STANDARD.decode(&signature).unwrap()).unwrap();
        assert!(text.contains(
            "trusted comment: timestamp:123\tfile:Kudzu_9.9.9-beta.2_universal.app.tar.gz"
        ));
        let url = "https://github.com/Stringsaeed/kudzu/releases/download/v9.9.9-beta.2/Kudzu_9.9.9-beta.2_universal.app.tar.gz";
        let feed = crate::feed::manifest(
            "9.9.9-beta.2",
            &signature,
            url,
            "notes",
            "2026-11-11T00:00:00Z",
        );
        assert_eq!(feed.as_object().unwrap().len(), 4);
        assert_eq!(feed["platforms"].as_object().unwrap().len(), 2);
        let public =
            minisign_verify::PublicKey::decode(&pair.pk.to_box().unwrap().to_string()).unwrap();
        for platform in ["darwin-aarch64", "darwin-x86_64"] {
            let p = &feed["platforms"][platform];
            assert_eq!(
                verify::verify(
                    &public,
                    bytes,
                    p["signature"].as_str().unwrap(),
                    feed["version"].as_str().unwrap(),
                    p["url"].as_str().unwrap()
                )
                .unwrap(),
                "9.9.9-beta.2"
            );
        }
        assert!(verify::verify(&public, b"tampered", &signature, "9.9.9-beta.2", url).is_err());
        assert!(archive_signature(bytes, &key, "wrong", "9.9.9", 123).is_err());
    }
}
