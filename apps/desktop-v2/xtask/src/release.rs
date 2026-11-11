use crate::{bundle, dmg, feed, sign, version};
use std::os::unix::fs::PermissionsExt;
use std::{
    env, fs,
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
fn required(name: &str) -> Result<String> {
    env::var(name)
        .ok()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("{name} required").into())
}
fn notarize(path: &Path, staple: &Path) -> Result<()> {
    // Capture output: external tools can echo credentials in diagnostics.
    let result = Command::new("xcrun")
        .args(["notarytool", "submit"])
        .arg(path)
        .arg("--key")
        .arg(required("APPLE_API_KEY_PATH")?)
        .arg("--key-id")
        .arg(required("APPLE_API_KEY")?)
        .arg("--issuer")
        .arg(required("APPLE_API_ISSUER")?)
        .args(["--wait", "--timeout", "30m"])
        .output()?;
    if !result.status.success() {
        return Err("notarization failed; owner should inspect Apple's submission log".into());
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
          preflight: design manifest; profile app id/container\n\
          temporary keychain: create, unlock, import P12, partition list, search list\n\
          bundle prod universal: arm64 + x86_64, minos 14.0\n\
          hardened codesign; verify team/bundle requirement, profile certificate and entitlements\n\
          notarize and staple app; create-dmg 1.2.3, background, 660x440, icons 170,220 / 490,220\n\
          sign, notarize and staple DMG; spctl assess app and DMG\n\
          tar Kudzu.app; base64 minisign signature bound to versioned filename\n\
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
    let version = version::parse(fs::read_to_string(root.join("version.txt"))?.trim())?.to_string();
    let identity = required("APPLE_SIGNING_IDENTITY")?;
    let skip = env::var("SKIP_NOTARIZE").as_deref() == Ok("1");
    if identity == "-" && !skip {
        return Err("ad-hoc signing requires SKIP_NOTARIZE=1".into());
    }
    let key = required("KUDZU_UPDATE_KEY")?;
    let password = env::var("KUDZU_UPDATE_KEY_PASSWORD").unwrap_or_default();
    if !skip {
        for name in ["APPLE_API_KEY_PATH", "APPLE_API_KEY", "APPLE_API_ISSUER"] {
            required(name)?;
        }
    }
    command(Command::new("bash").args(["scripts/release/build.sh", "--preflight"]))?;
    let private = root
        .join("target")
        .join(format!("release-private-{}", std::process::id()));
    fs::create_dir(&private)?;
    fs::set_permissions(&private, fs::Permissions::from_mode(0o700))?;
    let private = PrivateDir(private);
    let keychain = sign::Keychain::import(&private.0)?;
    let result = (|| {
        env::set_var("KUDZU_SIGN", &identity);
        let app = bundle::build(bundle::Variant::Prod, true)?;
        let mut codesign = Command::new("codesign");
        codesign
            .args(["--force", "--options", "runtime", "--sign"])
            .arg(&identity);
        if identity != "-" {
            codesign.arg("--timestamp");
        }
        if root.join("assets/Kudzu.provisionprofile").is_file() {
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
        if !skip {
            let zip = private.0.join("Kudzu.zip");
            command(
                Command::new("ditto")
                    .args(["-c", "-k", "--keepParent"])
                    .arg(&app)
                    .arg(&zip),
            )?;
            notarize(&zip, &app)?;
        }
        let dist = root.join("dist");
        if dist.exists() {
            return Err("dist already exists; move it aside before releasing".into());
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
        command(
            Command::new("codesign")
                .args(["--force", "--sign"])
                .arg(&identity)
                .arg(&image),
        )?;
        if !skip {
            notarize(&image, &image)?;
        }
        command(
            Command::new("bash")
                .arg("scripts/release/build.sh")
                .arg("--verify")
                .arg(&app)
                .arg(&image),
        )?;
        let archive_name = format!("Kudzu_{version}_universal.app.tar.gz");
        let archive = dist.join(&archive_name);
        command(
            Command::new("tar")
                .arg("-czf")
                .arg(&archive)
                .arg("-C")
                .arg(app.parent().unwrap())
                .arg("Kudzu.app"),
        )?;
        let now = chrono::Utc::now();
        let signature = sign::archive_signature(
            &fs::read(&archive)?,
            &key,
            &password,
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
    if let Some(keychain) = keychain {
        keychain.close()?;
    }
    result
}
