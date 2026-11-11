use crate::{archive, bundle, dmg, feed, sign, version};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::{
    collections::HashMap,
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned()
}
pub fn command(cmd: &mut Command) -> Result<()> {
    if !cmd.status()?.success() {
        return Err(format!(
            "release stage failed: {}",
            cmd.get_program().to_string_lossy()
        )
        .into());
    }
    Ok(())
}
/// Everything that can unlock the signing identity, an Apple account, the updater key or GitHub.
/// `Secrets::take` reads these once and removes them from this process's environment, and every
/// cargo child strips them again, so no build script, proc macro or helper process inherits them.
pub const SECRET_VARS: &[&str] = &[
    "APPLE_CERTIFICATE",
    "APPLE_CERTIFICATE_PASSWORD",
    "APPLE_API_KEY",
    "APPLE_API_ISSUER",
    "APPLE_API_KEY_P8",
    "APPLE_API_KEY_PATH",
    "KUDZU_UPDATE_KEY",
    "KUDZU_UPDATE_KEY_PASSWORD",
    "TAURI_SIGNING_PRIVATE_KEY",
    "TAURI_SIGNING_PRIVATE_KEY_PASSWORD",
    "GITHUB_TOKEN",
    "GH_TOKEN",
];

fn required(name: &str) -> Result<String> {
    env::var(name)
        .ok()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("{name} required").into())
}
/// Reads the named variables, then removes every one of them from the process environment.
fn take_env(names: &[&str]) -> HashMap<String, String> {
    let mut taken = HashMap::new();
    for name in names {
        if let Ok(value) = env::var(name) {
            if !value.is_empty() {
                taken.insert((*name).to_owned(), value);
            }
        }
        env::remove_var(name);
    }
    taken
}

enum ApiKey {
    /// The `.p8` contents (the CI secret). Written only after the build, into the private directory.
    Contents(String),
    /// A `.p8` file the owner already has, for local runs.
    File(PathBuf),
}
struct Notary {
    key: ApiKey,
    key_id: String,
    issuer: String,
}
struct Secrets {
    certificate: Option<(String, String)>,
    update_key: String,
    update_password: String,
    notary: Option<Notary>,
}
impl Secrets {
    fn take(skip_notarize: bool) -> Result<Self> {
        Self::from_vars(take_env(SECRET_VARS), skip_notarize)
    }
    fn from_vars(vars: HashMap<String, String>, skip_notarize: bool) -> Result<Self> {
        let need = |name: &str| {
            vars.get(name)
                .cloned()
                .ok_or_else(|| format!("{name} required"))
        };
        let certificate = match vars.get("APPLE_CERTIFICATE") {
            Some(certificate) => Some((certificate.clone(), need("APPLE_CERTIFICATE_PASSWORD")?)),
            None => None,
        };
        let notary = if skip_notarize {
            None
        } else {
            let key = match (vars.get("APPLE_API_KEY_P8"), vars.get("APPLE_API_KEY_PATH")) {
                (Some(contents), _) => ApiKey::Contents(contents.clone()),
                (None, Some(path)) => ApiKey::File(path.into()),
                (None, None) => {
                    return Err("APPLE_API_KEY_P8 or APPLE_API_KEY_PATH required".into())
                }
            };
            Some(Notary {
                key,
                key_id: need("APPLE_API_KEY")?,
                issuer: need("APPLE_API_ISSUER")?,
            })
        };
        Ok(Self {
            certificate,
            update_key: need("KUDZU_UPDATE_KEY")?,
            update_password: vars
                .get("KUDZU_UPDATE_KEY_PASSWORD")
                .cloned()
                .unwrap_or_default(),
            notary,
        })
    }
}

/// `notarytool` echoes paths and credentials-adjacent detail, so only the submission id and the
/// status are ever shown.
fn notary_summary(output: &str) -> String {
    output
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("id:") || line.starts_with("status:"))
        .collect::<Vec<_>>()
        .join("\n")
}
fn notarize(notary: &Notary, key_path: &Path, path: &Path, staple: &Path) -> Result<()> {
    // Capture output: external tools can echo credentials in diagnostics.
    let result = Command::new("xcrun")
        .args(["notarytool", "submit"])
        .arg(path)
        .arg("--key")
        .arg(key_path)
        .arg("--key-id")
        .arg(&notary.key_id)
        .arg("--issuer")
        .arg(&notary.issuer)
        .args(["--wait", "--timeout", "30m"])
        .output()?;
    let summary = notary_summary(&String::from_utf8_lossy(&result.stdout));
    if !summary.is_empty() {
        println!("notarytool {}:\n{summary}", path.display());
    }
    if !result.status.success() {
        return Err(
            "notarization failed; owner should inspect Apple's submission log (xcrun notarytool log <id>)"
                .into(),
        );
    }
    command(
        Command::new("xcrun")
            .args(["stapler", "staple"])
            .arg(staple),
    )?;
    command(
        Command::new("xcrun")
            .args(["stapler", "validate"])
            .arg(staple),
    )
}

/// Reports a stage failure and a cleanup failure together; either alone must not hide the other.
fn combine(result: Result<()>, cleanup: Result<()>) -> Result<()> {
    match (result, cleanup) {
        (Err(error), Err(cleanup)) => {
            Err(format!("{error}; additionally, release keychain cleanup failed: {cleanup}").into())
        }
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Ok(()), Ok(())) => Ok(()),
    }
}

fn runner_temp() -> Option<PathBuf> {
    env::var_os("RUNNER_TEMP")
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
}
/// Directories that may hold `release-private-*`: the runner's temp directory and `target/`.
pub fn private_root_candidates() -> Vec<PathBuf> {
    runner_temp()
        .into_iter()
        .chain([root().join("target")])
        .collect()
}
struct PrivateDir(PathBuf);
impl Drop for PrivateDir {
    fn drop(&mut self) {
        if !self.0.join("keychain-search-list.json").exists()
            && !self.0.join("release.keychain-db").exists()
        {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

pub fn run(args: &[String]) -> Result<()> {
    if args == ["cleanup"] {
        return sign::cleanup();
    }
    if args == ["--dry-run"] {
        println!(
            "DRY RUN: no credentials read and no release commands executed\n\
          read secrets once and remove them from the environment; no child process inherits them\n\
          preflight: version.txt canonical; dist/ absent; design manifest; profile app id/container;\n\
          \x20 unlock updater key and require the production public key (6E52269F4241F007)\n\
          bundle prod universal (--locked, no credentials present): arm64 + x86_64, minos 14.0\n\
          temporary keychain: create, unlock, re-wrap P12 under a one-time password, import (codesign only),\n\
          \x20 partition list, search list\n\
          hardened codesign; verify team/bundle requirement, profile certificate and entitlements\n\
          notarize and staple app; create-dmg 1.2.3, background, 660x440, icons 170,220 / 490,220\n\
          sign (timestamped), notarize and staple DMG; spctl assess app and DMG; verify DMG team\n\
          tar Kudzu.app (ustar, no AppleDouble or xattrs); sign, then verify against the production key;\n\
          \x20 base64 minisign signature bound to versioned filename\n\
          latest.json (both darwin platforms); SHA256SUMS.txt\n\
          restore keychain search list and delete temporary keychain"
        );
        return Ok(());
    }
    if !args.is_empty() {
        return Err("usage: cargo xtask release [--dry-run|cleanup]".into());
    }
    let root = root();
    env::set_current_dir(&root)?;
    // Force the S03 bundler and all child cargo invocations onto this worktree's cache.
    env::set_var("CARGO_TARGET_DIR", root.join("target"));
    env::set_var("MACOSX_DEPLOYMENT_TARGET", "14.0");
    let version = version::canonical(&fs::read_to_string(root.join("version.txt"))?)?;
    let identity = required("APPLE_SIGNING_IDENTITY")?;
    let skip = env::var("SKIP_NOTARIZE").as_deref() == Ok("1");
    if identity == "-" && !skip {
        return Err("ad-hoc signing requires SKIP_NOTARIZE=1".into());
    }
    // From here no child process (cargo, build scripts, proc macros, bash, tar) can read a secret.
    let secrets = Secrets::take(skip)?;

    // Preflight: everything that can fail cheaply fails before the long stages.
    command(Command::new("bash").args(["scripts/release/build.sh", "--preflight"]))?;
    let dist = root.join("dist");
    if dist.exists() {
        return Err("dist already exists; move it aside before releasing".into());
    }
    let update_key = sign::UpdateKey::unlock(&secrets.update_key, &secrets.update_password)?;
    // An ad-hoc build is never installable, so it may use a throwaway key; a real one may not.
    let expected = if identity == "-" {
        update_key.public_key()?
    } else {
        sign::production_public_key()?
    };
    sign::check_update_key(&update_key, &expected)?;

    // Build before any signing credential exists on this machine.
    let app = bundle::assemble(bundle::Variant::Prod, true)?;

    let base = runner_temp().unwrap_or_else(|| root.join("target"));
    fs::create_dir_all(&base)?;
    let private = base.join(format!("release-private-{}", std::process::id()));
    fs::create_dir(&private)?;
    fs::set_permissions(&private, fs::Permissions::from_mode(0o700))?;
    let private = PrivateDir(private);
    let keychain = sign::Keychain::import(
        &private.0,
        secrets
            .certificate
            .as_ref()
            .map(|(c, p)| (c.as_str(), p.as_str())),
    )?;
    let result = (|| {
        let notary_key = match &secrets.notary {
            Some(Notary {
                key: ApiKey::Contents(contents),
                ..
            }) => {
                let path = private.0.join("AuthKey.p8");
                fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(&path)?
                    .write_all(contents.as_bytes())?;
                path
            }
            Some(Notary {
                key: ApiKey::File(path),
                ..
            }) => path.clone(),
            None => PathBuf::new(),
        };
        let mut codesign = Command::new("codesign");
        codesign
            .args(["--force", "--options", "runtime", "--sign"])
            .arg(&identity);
        if identity != "-" {
            codesign.arg("--timestamp");
        }
        if bundle::profile_path().is_file() {
            codesign.args(["--entitlements", "assets/Kudzu.icloud.entitlements"]);
        }
        command(codesign.arg(&app))?;
        // Verify profile/certificate and minos before making an Apple submission.
        command(
            Command::new("bash")
                .arg("scripts/release/build.sh")
                .arg("--verify-app")
                .arg(&app),
        )?;
        if let Some(notary) = &secrets.notary {
            let zip = private.0.join("Kudzu.zip");
            command(
                Command::new("ditto")
                    .args(["-c", "-k", "--keepParent"])
                    .arg(&app)
                    .arg(&zip),
            )?;
            notarize(notary, &notary_key, &zip, &app)?;
        }
        fs::create_dir(&dist)?;
        let source = private.0.join("dmg");
        fs::create_dir(&source)?;
        command(
            Command::new("ditto")
                .arg(&app)
                .arg(source.join("Kudzu.app")),
        )?;
        let image = dist.join(format!("Kudzu_{version}_universal.dmg"));
        dmg::build(&source, &image)?;
        let mut codesign = Command::new("codesign");
        codesign.args(["--force", "--sign"]).arg(&identity);
        if identity != "-" {
            codesign.arg("--timestamp");
        }
        command(codesign.arg(&image))?;
        if let Some(notary) = &secrets.notary {
            notarize(notary, &notary_key, &image, &image)?;
        }
        command(
            Command::new("bash")
                .arg("scripts/release/build.sh")
                .arg("--verify")
                .arg(&app)
                .arg(&image),
        )?;
        let archive_name = format!("Kudzu_{version}_universal.app.tar.gz");
        let archive_path = dist.join(&archive_name);
        archive::create(app.parent().unwrap(), "Kudzu.app", &archive_path)?;
        let now = chrono::Utc::now();
        let signature = sign::archive_signature(
            &update_key,
            &expected,
            &fs::read(&archive_path)?,
            &version,
            now.timestamp(),
        )?;
        fs::write(dist.join(format!("{archive_name}.sig")), &signature)?;
        let repo = env::var("GITHUB_REPOSITORY").unwrap_or_else(|_| "Stringsaeed/kudzu".into());
        let url = format!("https://github.com/{repo}/releases/download/v{version}/{archive_name}");
        let notes = match env::var("RELEASE_NOTES_FILE") {
            Ok(path) if !path.is_empty() => fs::read_to_string(path)?,
            _ => format!("Kudzu {version}"),
        };
        let manifest = feed::manifest(&version, &signature, &url, notes.trim(), &now.to_rfc3339());
        fs::write(
            dist.join("latest.json"),
            serde_json::to_string_pretty(&manifest)? + "\n",
        )?;
        let sums = Command::new("shasum")
            .current_dir(&dist)
            .args(["-a", "256"])
            .arg(image.file_name().unwrap())
            .arg(&archive_name)
            .arg(format!("{archive_name}.sig"))
            .arg("latest.json")
            .output()?;
        if !sums.status.success() {
            return Err("checksums failed".into());
        }
        fs::write(dist.join("SHA256SUMS.txt"), sums.stdout)?;
        Ok(())
    })();
    combine(result, keychain.map_or(Ok(()), |keychain| keychain.close()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).into(), (*v).into()))
            .collect()
    }

    #[test]
    fn secrets_are_read_once_and_removed_from_the_environment() {
        let names = [
            "KUDZU_S70_TEST_SECRET",
            "KUDZU_S70_TEST_EMPTY",
            "KUDZU_S70_TEST_ABSENT",
        ];
        env::set_var(names[0], "value");
        env::set_var(names[1], "");
        let taken = take_env(&names);
        assert_eq!(taken.len(), 1);
        assert_eq!(taken[names[0]], "value");
        assert!(names.iter().all(|n| env::var_os(n).is_none()));

        // The production list, set to recognizable values, leaves no trace for a child.
        for name in SECRET_VARS {
            env::set_var(name, "leak");
        }
        let taken = take_env(SECRET_VARS);
        assert_eq!(taken.len(), SECRET_VARS.len());
        let child = Command::new("/usr/bin/env").output().unwrap();
        let listing = String::from_utf8_lossy(&child.stdout);
        for name in SECRET_VARS {
            assert!(!listing.contains(&format!("{name}=")), "{name} inherited");
        }
    }

    #[test]
    fn required_secrets_are_checked_up_front() {
        let full = [
            ("KUDZU_UPDATE_KEY", "key"),
            ("APPLE_API_KEY", "id"),
            ("APPLE_API_ISSUER", "issuer"),
            ("APPLE_API_KEY_P8", "p8"),
        ];
        let secrets = Secrets::from_vars(vars(&full), false).unwrap();
        assert!(secrets.certificate.is_none());
        assert!(matches!(
            secrets.notary.unwrap().key,
            ApiKey::Contents(contents) if contents == "p8"
        ));
        for missing in 0..full.len() {
            let mut partial = full.to_vec();
            partial.remove(missing);
            assert!(
                Secrets::from_vars(vars(&partial), false).is_err(),
                "{}",
                full[missing].0
            );
        }
        // Skipping notarization needs only the updater key.
        assert!(Secrets::from_vars(vars(&full[..1]), true)
            .unwrap()
            .notary
            .is_none());
        assert!(Secrets::from_vars(vars(&[]), true).is_err());
        // A certificate without its password is refused before anything is built.
        let mut with_certificate = full.to_vec();
        with_certificate.push(("APPLE_CERTIFICATE", "p12"));
        assert!(Secrets::from_vars(vars(&with_certificate), false).is_err());
        with_certificate.push(("APPLE_CERTIFICATE_PASSWORD", "pw"));
        assert!(Secrets::from_vars(vars(&with_certificate), false)
            .unwrap()
            .certificate
            .is_some());
    }

    #[test]
    fn only_the_submission_id_and_status_survive_notarytool_output() {
        let output = "Conducting pre-submission checks for /secret/path/Kudzu.zip...\n\
            Submission ID received\n  id: 11111111-2222-3333-4444-555555555555\n\
            Successfully uploaded file\n  id: 11111111-2222-3333-4444-555555555555\n  path: /secret/path/Kudzu.zip\n\
            Current status: Accepted.....\nProcessing complete\n  id: 11111111-2222-3333-4444-555555555555\n  status: Accepted\n";
        let summary = notary_summary(output);
        assert_eq!(summary.lines().count(), 4);
        assert!(summary.ends_with("status: Accepted"));
        assert!(!summary.contains("/secret/path") && !summary.contains("Current status"));
        assert_eq!(notary_summary("error: key abc was rejected\n"), "");
    }

    #[test]
    fn a_failed_stage_and_a_failed_cleanup_are_both_reported() {
        let stage: Result<()> = Err("stage failed".into());
        let cleanup: Result<()> = Err("keychain still exists".into());
        let both = combine(stage, cleanup).unwrap_err().to_string();
        assert!(
            both.contains("stage failed") && both.contains("keychain still exists"),
            "{both}"
        );
        assert_eq!(
            combine(Err("stage failed".into()), Ok(()))
                .unwrap_err()
                .to_string(),
            "stage failed"
        );
        assert_eq!(
            combine(Ok(()), Err("keychain still exists".into()))
                .unwrap_err()
                .to_string(),
            "keychain still exists"
        );
        assert!(combine(Ok(()), Ok(())).is_ok());
    }

    #[test]
    fn the_cask_zaps_the_login_item_this_app_writes() {
        let dir = root()
            .join("target")
            .join(format!("s70-cask-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let status = Command::new("bash")
            .arg(root().join("scripts/release/update-cask.sh"))
            .args(["0.7.1", &"a".repeat(64)])
            .arg(&dir)
            .stdout(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
        let cask = fs::read_to_string(dir.join("Casks/kudzu.rb")).unwrap();
        // S36 names the login item after the bundle id (login_item.rs), not after Tauri's "Kudzu".
        assert!(cask.contains("\"~/Library/LaunchAgents/sh.saeed.kudzu.plist\""));
        assert!(cask.contains("\"~/Library/Logs/sh.saeed.kudzu\""));
        assert!(!cask.contains("LaunchAgents/Kudzu.plist"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_updater_archive_has_no_apple_metadata() {
        let dir = root()
            .join("target")
            .join(format!("s70-archive-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let app = dir.join("Kudzu.app/Contents");
        fs::create_dir_all(&app).unwrap();
        fs::write(app.join("Info.plist"), "plist").unwrap();
        for path in [dir.join("Kudzu.app"), app.clone(), app.join("Info.plist")] {
            let status = Command::new("/usr/bin/xattr")
                .args(["-w", "com.kudzu.s70-test", "value"])
                .arg(&path)
                .status()
                .unwrap();
            assert!(status.success());
        }
        let archive = dir.join("out.tar.gz");
        archive::create(&dir, "Kudzu.app", &archive).unwrap();
        let tar = Command::new("/usr/bin/gzip")
            .arg("-dc")
            .arg(&archive)
            .output()
            .unwrap();
        let listing = String::from_utf8_lossy(&tar.stdout);
        assert!(listing.contains("Kudzu.app/Contents/Info.plist"));
        assert!(!listing.contains("._") && !listing.contains("xattr"));
        fs::remove_dir_all(dir).unwrap();
    }
}
