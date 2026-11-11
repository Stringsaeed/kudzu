use crate::release::Result;
use base64::{engine::general_purpose::STANDARD, Engine};
use minisign::{PublicKey, PublicKeyBox, SecretKey, SecretKeyBox, SignatureBox};
use std::os::unix::fs::OpenOptionsExt;
use std::{
    fs,
    io::{Cursor, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

/// The production updater public key, key id `6E52269F4241F007`. A byte-identical copy of
/// `PRODUCTION_PUBKEY` in `kudzu-app`'s `updates/verify.rs`, the key S36 compiles in. xtask may
/// not depend on `kudzu-*` crates (`cargo xtask check-deps`), so a test pins the two together.
pub const PRODUCTION_PUBKEY: &str =
    "untrusted comment: minisign public key: 6E52269F4241F007\nRWQH8EFCnyZSbuM5BGaktuexN/70p9Q1ZPCwwNt5ilvPkDnn57keAIOC\n";

pub fn production_public_key() -> Result<PublicKey> {
    public_key(PRODUCTION_PUBKEY)
}
fn public_key(text: &str) -> Result<PublicKey> {
    Ok(PublicKeyBox::from_string(text)
        .and_then(PublicKeyBox::into_public_key)
        .map_err(|_| "invalid public key")?)
}
/// The key id as minisign prints it: the keynum bytes read as a little-endian number.
fn key_id(key: &PublicKey) -> String {
    key.keynum().try_into().map_or_else(
        |_| "unknown".into(),
        |bytes| format!("{:016X}", u64::from_le_bytes(bytes)),
    )
}

/// An unlocked updater signing key. It is never printed, and its errors carry no key material.
pub struct UpdateKey(SecretKey);
impl UpdateKey {
    /// `wrapped` is the base64 minisign key text that Tauri's CLI writes (`TAURI_SIGNING_PRIVATE_KEY`).
    pub fn unlock(wrapped: &str, password: &str) -> Result<Self> {
        let decoded = STANDARD
            .decode(wrapped.trim())
            .map_err(|_| "invalid wrapped updater key")?;
        let text = std::str::from_utf8(&decoded).map_err(|_| "invalid updater key text")?;
        let key = SecretKeyBox::from_string(text)
            .and_then(|b| b.into_secret_key(Some(password.to_owned())))
            .map_err(|_| "cannot unlock updater key (wrong key or password)")?;
        Ok(Self(key))
    }
    pub fn public_key(&self) -> Result<PublicKey> {
        Ok(PublicKey::from_secret_key(&self.0).map_err(|_| "updater key has no public half")?)
    }
    fn sign(&self, bytes: &[u8], comment: &str) -> Result<SignatureBox> {
        Ok(
            minisign::sign(None, &self.0, Cursor::new(bytes), Some(comment), None)
                .map_err(|_| "updater signing failed")?,
        )
    }
}

fn verify(key: &PublicKey, signature: &SignatureBox, bytes: &[u8]) -> Result<()> {
    minisign::verify(key, signature, Cursor::new(bytes), true, false, false)
        .map_err(|_| "updater signature does not verify against the expected public key")?;
    Ok(())
}

/// Release preflight: the unlocked key must be the pair of `expected`. Comparing the embedded
/// public half is not enough, since a stale half would pass, so this also signs a probe and
/// verifies it against `expected`. A wrong key or password then fails in seconds instead of after
/// notarization, and never produces a green run that every installed copy rejects.
pub fn check_update_key(key: &UpdateKey, expected: &PublicKey) -> Result<()> {
    let embedded = key.public_key()?;
    if embedded.to_bytes() != expected.to_bytes() {
        return Err(format!(
            "updater key is not the expected key: key id {} (expected {})",
            key_id(&embedded),
            key_id(expected)
        )
        .into());
    }
    let probe = b"kudzu updater key probe";
    verify(expected, &key.sign(probe, "probe")?, probe)
}

/// Signs the archive, proves the signature against `expected` before anything is written, and
/// returns the base64-wrapped signature that the feed carries.
pub fn archive_signature(
    key: &UpdateKey,
    expected: &PublicKey,
    bytes: &[u8],
    version: &str,
    timestamp: i64,
) -> Result<String> {
    let comment = format!("timestamp:{timestamp}\tfile:Kudzu_{version}_universal.app.tar.gz");
    let signature = key.sign(bytes, &comment)?;
    verify(expected, &signature, bytes)?;
    Ok(STANDARD.encode(signature.to_string()))
}

// Secret-bearing commands never inherit output or appear in errors.
fn security(args: &[&str]) -> Result<String> {
    let output = Command::new("security").args(args).output()?;
    if !output.status.success() {
        return Err("release keychain operation failed (details suppressed)".into());
    }
    Ok(String::from_utf8(output.stdout)?)
}

const OPENSSL: &str = "/usr/bin/openssl";
fn random_password() -> Result<String> {
    let password = Command::new(OPENSSL)
        .args(["rand", "-hex", "32"])
        .output()?;
    if !password.status.success() {
        return Err("password generation failed".into());
    }
    Ok(String::from_utf8(password.stdout)?.trim().to_owned())
}

/// Re-wraps a P12 under a one-time password, so that only a throwaway reaches `security import`'s
/// argv, where any local user can read it. Passwords travel in the child's environment. The
/// intermediate PEM keeps the key encrypted under the one-time password (LibreSSL's `-export`
/// cannot read the key and the certificates from one pipe), so no plaintext key is ever written.
fn rewrap_p12(source: &Path, old_password: &str, target: &Path, new_password: &str) -> Result<()> {
    let opened = Command::new(OPENSSL)
        .args([
            "pkcs12",
            "-passin",
            "env:KUDZU_P12_OLD",
            "-passout",
            "env:KUDZU_P12_NEW",
            "-in",
        ])
        .arg(source)
        .env("KUDZU_P12_OLD", old_password)
        .env("KUDZU_P12_NEW", new_password)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()?;
    if !opened.status.success() {
        return Err("cannot open the signing certificate (wrong password?)".into());
    }
    let pem = target.with_extension("pem");
    let exported = (|| {
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&pem)?
            .write_all(&opened.stdout)?;
        let status = Command::new(OPENSSL)
            .args([
                "pkcs12",
                "-export",
                "-passin",
                "env:KUDZU_P12_NEW",
                "-passout",
                "env:KUDZU_P12_NEW",
                "-in",
            ])
            .arg(&pem)
            .arg("-out")
            .arg(target)
            .env("KUDZU_P12_NEW", new_password)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err("cannot re-wrap the signing certificate".into())
        }
    })();
    let _ = fs::remove_file(&pem);
    exported
}

type Run = fn(&[&str]) -> Result<String>;
type Rewrap = fn(&Path, &str, &Path, &str) -> Result<()>;
pub struct Keychain {
    path: PathBuf,
    original: Vec<String>,
    run: Run,
}
impl Keychain {
    /// `certificate` is the base64 P12 and its password; `None` leaves the keychains alone.
    pub fn import(dir: &Path, certificate: Option<(&str, &str)>) -> Result<Option<Self>> {
        let Some((certificate, password)) = certificate else {
            return Ok(None);
        };
        Self::create(dir, certificate, password, security, rewrap_p12).map(Some)
    }
    fn create(
        dir: &Path,
        certificate: &str,
        cert_password: &str,
        run: Run,
        rewrap: Rewrap,
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
        let password = random_password()?;
        run(&["create-keychain", "-p", &password, path])?;
        run(&["set-keychain-settings", "-lut", "21600", path])?;
        run(&["unlock-keychain", "-p", &password, path])?;
        let p12 = dir.join("certificate.p12");
        fs::write(
            &p12,
            STANDARD
                .decode(certificate.trim())
                .map_err(|_| "invalid certificate encoding")?,
        )?;
        let one_time = random_password()?;
        let wrapped = dir.join("import.p12");
        let rewrapped = rewrap(&p12, cert_password, &wrapped, &one_time);
        fs::remove_file(&p12)?;
        rewrapped?;
        // Only /usr/bin/codesign may use the key. Allowing /usr/bin/security would let it export it.
        let imported = run(&[
            "import",
            wrapped.to_str().unwrap(),
            "-k",
            path,
            "-P",
            &one_time,
            "-T",
            "/usr/bin/codesign",
        ]);
        fs::remove_file(&wrapped)?;
        imported?;
        run(&[
            "set-key-partition-list",
            "-S",
            "apple-tool:,apple:,codesign:",
            "-s",
            "-k",
            &password,
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

/// Every directory that may hold `release-private-*` folders: the runner's temp directory and
/// the workspace's `target/`.
fn private_roots() -> Vec<PathBuf> {
    let mut roots = crate::release::private_root_candidates();
    roots.retain(|p| p.exists());
    roots
}

pub fn cleanup() -> Result<()> {
    for root in private_roots() {
        for entry in fs::read_dir(root)? {
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
    // Throwaway key in the format Tauri's CLI writes; it signs nothing real. See tests/fixtures.
    const FIXTURE_KEY: &str = include_str!("../tests/fixtures/throwaway-tauri-key");
    const FIXTURE_PUBLIC: &str = include_str!("../tests/fixtures/throwaway-tauri-key.pub");
    const FIXTURE_PASSWORD: &str = include_str!("../tests/fixtures/throwaway-tauri-key.password");
    thread_local! {
        static CALLS: std::cell::RefCell<Vec<Vec<String>>> = const { std::cell::RefCell::new(Vec::new()) };
        static FAIL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
        static REWRAP: std::cell::RefCell<Vec<(String, String)>> = const { std::cell::RefCell::new(Vec::new()) };
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
    fn fake_rewrap(source: &Path, old: &str, target: &Path, new: &str) -> Result<()> {
        assert!(source.exists());
        REWRAP.with(|r| r.borrow_mut().push((old.into(), new.into())));
        fs::write(target, "rewrapped")?;
        Ok(())
    }
    fn unique_dir(name: &str) -> PathBuf {
        let dir = crate::release::root()
            .join("target")
            .join(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn keychain_restores_search_list_on_success_and_import_failure() {
        let dir = unique_dir("s70-keychain-test");
        for fail in [false, true] {
            FAIL.with(|f| f.set(fail));
            CALLS.with(|c| c.borrow_mut().clear());
            let result = Keychain::create(
                &dir,
                &STANDARD.encode("throwaway p12"),
                "throwaway",
                fake_security,
                fake_rewrap,
            );
            if fail {
                assert!(result.is_err());
            } else {
                result.unwrap().close().unwrap();
            }
            assert!(!dir.join("release.keychain-db").exists());
            assert!(!dir.join("keychain-search-list.json").exists());
            assert!(!dir.join("certificate.p12").exists());
            assert!(!dir.join("import.p12").exists());
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
    fn the_real_p12_password_never_reaches_argv_and_only_codesign_may_use_the_key() {
        let dir = unique_dir("s70-p12-argv-test");
        CALLS.with(|c| c.borrow_mut().clear());
        REWRAP.with(|r| r.borrow_mut().clear());
        FAIL.with(|f| f.set(false));
        Keychain::create(
            &dir,
            &STANDARD.encode("throwaway p12"),
            "the-real-p12-password",
            fake_security,
            fake_rewrap,
        )
        .unwrap()
        .close()
        .unwrap();
        let (old, new) = REWRAP.with(|r| r.borrow()[0].clone());
        assert_eq!(old, "the-real-p12-password");
        assert_ne!(new, old);
        assert_eq!(new.len(), 64, "random one-time password");
        CALLS.with(|calls| {
            let calls = calls.borrow();
            assert!(calls
                .iter()
                .flatten()
                .all(|arg| !arg.contains("the-real-p12-password")));
            let import = calls.iter().find(|c| c[0] == "import").unwrap();
            let at = |flag: &str| import.iter().position(|a| a == flag).unwrap();
            assert_eq!(import[at("-P") + 1], new);
            assert_eq!(
                import.iter().filter(|a| a.as_str() == "-T").count(),
                1,
                "{import:?}"
            );
            assert_eq!(import[at("-T") + 1], "/usr/bin/codesign");
            assert!(import.iter().all(|a| a != "/usr/bin/security"));
        });
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_real_p12_is_rewrapped_under_the_one_time_password_only() {
        let dir = unique_dir("s70-p12-rewrap-test");
        let openssl = |args: &[&str]| {
            let output = Command::new(OPENSSL).args(args).output().unwrap();
            output.status.success()
        };
        let (key, cert, old, new) = (
            dir.join("key.pem"),
            dir.join("cert.pem"),
            dir.join("old.p12"),
            dir.join("new.p12"),
        );
        assert!(openssl(&[
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-days",
            "1",
            "-subj",
            "/CN=Kudzu S70 throwaway",
            "-keyout",
            key.to_str().unwrap(),
            "-out",
            cert.to_str().unwrap(),
        ]));
        assert!(openssl(&[
            "pkcs12",
            "-export",
            "-inkey",
            key.to_str().unwrap(),
            "-in",
            cert.to_str().unwrap(),
            "-out",
            old.to_str().unwrap(),
            "-passout",
            "pass:original",
        ]));
        assert!(rewrap_p12(&old, "wrong", &new, "one-time").is_err());
        rewrap_p12(&old, "original", &new, "one-time").unwrap();
        let opens = |password: &str| {
            openssl(&[
                "pkcs12",
                "-in",
                new.to_str().unwrap(),
                "-noout",
                "-passin",
                &format!("pass:{password}"),
            ])
        };
        assert!(opens("one-time"));
        assert!(!opens("original"));
        assert!(
            !new.with_extension("pem").exists(),
            "intermediate PEM removed"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    fn fixture() -> (UpdateKey, PublicKey) {
        let key = UpdateKey::unlock(FIXTURE_KEY, FIXTURE_PASSWORD).unwrap();
        let public = public_key(
            std::str::from_utf8(&STANDARD.decode(FIXTURE_PUBLIC.trim()).unwrap()).unwrap(),
        )
        .unwrap();
        (key, public)
    }

    #[test]
    fn the_two_copies_of_the_production_key_are_byte_identical() {
        assert_eq!(PRODUCTION_PUBKEY, verify::PRODUCTION_PUBKEY);
        assert_eq!(
            STANDARD.encode(PRODUCTION_PUBKEY),
            verify::PRODUCTION_PUBKEY_TAURI_B64
        );
        assert_eq!(
            key_id(&production_public_key().unwrap()),
            "6E52269F4241F007"
        );
    }

    #[test]
    fn a_tauri_format_key_unlocks_signs_and_verifies_with_s31() {
        let (key, public) = fixture();
        assert!(UpdateKey::unlock(FIXTURE_KEY, "wrong").is_err());
        assert!(UpdateKey::unlock("not base64!", FIXTURE_PASSWORD).is_err());
        assert_eq!(key.public_key().unwrap().to_bytes(), public.to_bytes());
        check_update_key(&key, &public).unwrap();

        let bytes = b"throwaway archive payload";
        let signature = archive_signature(&key, &public, bytes, "9.9.9-beta.2", 123).unwrap();
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
        let verifier = minisign_verify::PublicKey::decode(
            std::str::from_utf8(&STANDARD.decode(FIXTURE_PUBLIC.trim()).unwrap()).unwrap(),
        )
        .unwrap();
        for platform in ["darwin-aarch64", "darwin-x86_64"] {
            let p = &feed["platforms"][platform];
            assert_eq!(
                verify::verify(
                    &verifier,
                    bytes,
                    p["signature"].as_str().unwrap(),
                    feed["version"].as_str().unwrap(),
                    p["url"].as_str().unwrap()
                )
                .unwrap(),
                "9.9.9-beta.2"
            );
        }
        assert!(verify::verify(&verifier, b"tampered", &signature, "9.9.9-beta.2", url).is_err());
    }

    #[test]
    fn a_key_that_is_not_the_production_key_is_refused_before_any_build() {
        let (key, own) = fixture();
        let production = production_public_key().unwrap();
        let error = check_update_key(&key, &production).unwrap_err().to_string();
        assert!(error.contains("6E52269F4241F007"), "{error}");
        assert!(error.contains("expected"), "{error}");
        // A signature made by another key cannot be written out either.
        assert!(archive_signature(&key, &production, b"bytes", "1.0.0", 1).is_err());
        let other = minisign::KeyPair::generate_unencrypted_keypair().unwrap();
        assert!(check_update_key(&key, &other.pk).is_err());
        check_update_key(&key, &own).unwrap();
    }

    #[test]
    fn a_tampered_archive_fails_the_post_sign_verification() {
        let (key, public) = fixture();
        let signature = key.sign(b"original", "comment").unwrap();
        assert!(verify(&public, &signature, b"original").is_ok());
        assert!(verify(&public, &signature, b"tampered").is_err());
    }
}
