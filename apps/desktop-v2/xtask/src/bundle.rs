use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Clone, Copy)]
pub enum Variant {
    Dev,
    Prod,
}
impl Variant {
    fn name(self) -> &'static str {
        match self {
            Self::Dev => "Kudzu Next",
            Self::Prod => "Kudzu",
        }
    }
    fn id(self) -> &'static str {
        match self {
            Self::Dev => "sh.saeed.kudzu.next",
            Self::Prod => "sh.saeed.kudzu",
        }
    }
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}
fn command(cmd: &mut Command) -> Result<()> {
    let status = cmd.status()?;
    if !status.success() {
        return Err(format!("{cmd:?} failed: {status}").into());
    }
    Ok(())
}
fn plist(variant: Variant, version: &str) -> String {
    include_str!("../../assets/Info.plist.in")
        .replace("@BUNDLE_ID@", variant.id())
        .replace("@NAME@", variant.name())
        .replace("@VERSION@", version)
}
pub fn build(variant: Variant, universal: bool) -> Result<PathBuf> {
    let root = root();
    let target = root.join("target");
    let archs = ["aarch64-apple-darwin", "x86_64-apple-darwin"];
    if universal {
        for arch in archs {
            command(Command::new("cargo").current_dir(&root).args([
                "build",
                "--release",
                "--target",
                arch,
                "-p",
                "kudzu",
            ]))?;
        }
    } else {
        let mut cmd = Command::new("cargo");
        cmd.current_dir(&root).args(build_args(variant));
        command(&mut cmd)?;
    }
    let app = target
        .join("bundle")
        .join(format!("{}.app", variant.name()));
    let contents = app.join("Contents");
    if app.exists() {
        fs::remove_dir_all(&app)?;
    }
    fs::create_dir_all(contents.join("MacOS"))?;
    fs::create_dir_all(contents.join("Resources/skills/kudzu"))?;
    let executable = contents.join("MacOS/Kudzu");
    if universal {
        command(
            Command::new("lipo")
                .arg("-create")
                .arg(target.join(archs[0]).join("release/kudzu"))
                .arg(target.join(archs[1]).join("release/kudzu"))
                .arg("-output")
                .arg(&executable),
        )?;
    } else {
        fs::copy(target.join(binary_path(variant)), &executable)?;
    }
    fs::write(
        contents.join("Info.plist"),
        plist(
            variant,
            fs::read_to_string(root.join("version.txt"))?.trim(),
        ),
    )?;
    fs::copy(
        root.join("assets/icons/icon.icns"),
        contents.join("Resources/icon.icns"),
    )?;
    fs::copy(
        root.join("skills/kudzu/SKILL.md"),
        contents.join("Resources/skills/kudzu/SKILL.md"),
    )?;
    let profile = root.join("assets/Kudzu.provisionprofile");
    if matches!(variant, Variant::Prod) && profile.is_file() {
        let signer = env::var_os("KUDZU_SIGN")
            .filter(|s| !s.is_empty())
            .ok_or("KUDZU_SIGN is required when embedding a provisioning profile")?;
        fs::copy(&profile, contents.join("embedded.provisionprofile"))?;
        command(
            Command::new("codesign")
                .args(["--force", "--options", "runtime", "--sign"])
                .arg(signer)
                .arg("--entitlements")
                .arg(root.join("assets/Kudzu.icloud.entitlements"))
                .arg(&app),
        )?;
    }
    if matches!(variant, Variant::Dev) {
        let signer = env::var_os("KUDZU_DEV_SIGN")
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "-".into());
        command(
            Command::new("codesign")
                .args(["--force", "--sign"])
                .arg(signer)
                .arg(&app),
        )?;
    }
    Ok(app)
}
fn build_args(variant: Variant) -> Vec<&'static str> {
    let mut args = vec!["build", "-p", "kudzu"];
    if matches!(variant, Variant::Prod) {
        args.push("--release");
    }
    args
}
fn binary_path(variant: Variant) -> &'static str {
    match variant {
        Variant::Dev => "debug/kudzu",
        Variant::Prod => "release/kudzu",
    }
}
pub fn parse(args: &[String]) -> Result<(Variant, bool)> {
    let mut variant = None;
    let mut universal = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--variant" => {
                variant = Some(match iter.next().map(String::as_str) {
                    Some("dev") => Variant::Dev,
                    Some("prod") => Variant::Prod,
                    _ => return Err("expected --variant dev|prod".into()),
                })
            }
            "--universal" => universal = true,
            _ => return Err(format!("unknown bundle argument: {arg}").into()),
        }
    }
    Ok((variant.ok_or("expected --variant dev|prod")?, universal))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn production_uses_release_binary() {
        assert_eq!(
            build_args(Variant::Prod),
            ["build", "-p", "kudzu", "--release"]
        );
        assert_eq!(build_args(Variant::Dev), ["build", "-p", "kudzu"]);
        assert_eq!(binary_path(Variant::Prod), "release/kudzu");
        assert_eq!(binary_path(Variant::Dev), "debug/kudzu");
    }
    #[test]
    fn plist_has_bundle_metadata() {
        let version = fs::read_to_string(root().join("version.txt")).unwrap();
        for variant in [Variant::Dev, Variant::Prod] {
            let text = plist(variant, version.trim());
            let value = plist::Value::from_reader_xml(text.as_bytes()).unwrap();
            let d = value.as_dictionary().unwrap();
            assert_eq!(d["CFBundleIdentifier"].as_string(), Some(variant.id()));
            assert_eq!(
                d["CFBundleShortVersionString"].as_string(),
                Some(version.trim())
            );
            assert_eq!(d["LSMinimumSystemVersion"].as_string(), Some("14.0"));
            assert_eq!(d["LSUIElement"].as_boolean(), Some(true));
            for key in [
                "NSSupportsSuddenTermination",
                "NSSupportsAutomaticTermination",
            ] {
                assert!(
                    matches!(d.get(key), None | Some(plist::Value::Boolean(false))),
                    "{key} must be absent or false"
                );
            }
        }
    }
}
