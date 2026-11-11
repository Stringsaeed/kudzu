//! Validated, bounded extraction beside the running bundle and a rollback-capable rename.
//! No constructor reads global identity or installs anything. S40 supplies the running version.
use kudzu_app::{
    error::PortError,
    ports::{InstallProbe, UpdateInstaller, UpdatePackage},
};
use semver::Version;
use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Mutex,
};

pub(crate) mod location;
mod swap;
use location::Scratch;

const MAX_ARCHIVE: u64 = 256 * 1024 * 1024;
const MAX_EXTRACTED: u64 = 1024 * 1024 * 1024;
const MAX_ENTRIES: usize = 10_000;
const MAX_TAR: u64 = MAX_EXTRACTED + MAX_ENTRIES as u64 * 1024 + 1024 * 1024;
const ROOT: &str = "Kudzu.app";

pub struct BundleInstaller {
    bundle: PathBuf,
    bundle_id: String,
    running: Version,
    operation: Mutex<()>,
}

impl BundleInstaller {
    /// `running_version` must come from the running binary, never a downloaded feed or disk app.
    pub fn new(bundle: PathBuf, bundle_id: &str, running_version: &str) -> Result<Self, PortError> {
        if !super::login_item::valid_id(bundle_id) {
            return Err(PortError::InvalidData);
        }
        Ok(Self {
            bundle,
            bundle_id: bundle_id.into(),
            running: Version::parse(running_version).map_err(|_| PortError::InvalidData)?,
            operation: Mutex::new(()),
        })
    }

    fn install_with(
        &self,
        package: &UpdatePackage,
        verify: impl FnOnce(&Path, &str) -> Result<(), PortError>,
        rename: impl Fn(&Path, &Path) -> io::Result<()>,
    ) -> Result<(), PortError> {
        self.install_using(package, verify, swap::exchange, rename)
    }

    fn install_using(
        &self,
        package: &UpdatePackage,
        verify: impl FnOnce(&Path, &str) -> Result<(), PortError>,
        swap: impl Fn(&Path, &Path) -> io::Result<()>,
        rename: impl Fn(&Path, &Path) -> io::Result<()>,
    ) -> Result<(), PortError> {
        let _operation = self.operation.lock().unwrap();
        let probe = self.probe()?;
        let version = Version::parse(&package.version).map_err(|_| PortError::InvalidData)?;
        if package.bundle_id != self.bundle_id || !version.cmp_precedence(&self.running).is_gt() {
            return Err(PortError::InvalidData);
        }
        if already_current(&self.bundle, &version) {
            return Ok(());
        }
        if probe.needs_move {
            return Err(PortError::PermissionDenied);
        }
        let mut stage = Scratch::for_bundle(&self.bundle)?;
        let snapshot = stage.0.join("archive.tar.gz");
        snapshot_archive(package, &snapshot)?;
        let tar = stage.0.join("validated.tar");
        normalize_archive(&snapshot, &tar)?;
        validate_tar(&tar, MAX_EXTRACTED, MAX_ENTRIES)?;
        let status = Command::new("/usr/bin/tar")
            .args(["-xf"])
            .arg(&tar)
            .arg("-C")
            .arg(&stage.0)
            .args(["--no-same-owner", "--no-same-permissions"])
            .env("COPYFILE_DISABLE", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(port_io)?;
        if !status.success() {
            return Err(PortError::InvalidData);
        }
        let extracted = stage.0.join(ROOT);
        validate_bundle(&extracted, package)?;
        verify(&extracted, &self.bundle_id)?;
        if already_current(&self.bundle, &version) {
            return Ok(());
        }
        swap::publish(&mut stage, &self.bundle, swap, rename).map_err(port_io)
    }
}

impl UpdateInstaller for BundleInstaller {
    fn probe(&self) -> Result<InstallProbe, PortError> {
        Ok(location::probe(&self.bundle))
    }

    fn install(&self, package: &UpdatePackage) -> Result<(), PortError> {
        self.install_with(package, verify_signature, |from, to| fs::rename(from, to))
    }
}

fn port_io(error: io::Error) -> PortError {
    PortError::Io(error.into())
}
fn already_current(bundle: &Path, version: &Version) -> bool {
    (|| {
        let mut bytes = Vec::new();
        File::open(bundle.join("Contents/Info.plist"))
            .ok()?
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .ok()?;
        if bytes.len() > 1024 * 1024 {
            return None;
        }
        let plist = plist::Value::from_reader(io::Cursor::new(bytes)).ok()?;
        Version::parse(
            plist
                .as_dictionary()?
                .get("CFBundleShortVersionString")?
                .as_string()?,
        )
        .ok()
    })()
    .is_some_and(|installed| !installed.cmp_precedence(version).is_lt())
}

fn snapshot_archive(package: &UpdatePackage, destination: &Path) -> Result<(), PortError> {
    let source = File::open(&package.archive).map_err(port_io)?;
    if !source.metadata().map_err(port_io)?.is_file() {
        return Err(PortError::InvalidData);
    }
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(destination)
        .map_err(port_io)?;
    let mut input = source.take(MAX_ARCHIVE + 1);
    let mut hash = blake3::Hasher::new();
    let mut buffer = [0; 32 * 1024];
    let mut size = 0;
    loop {
        let count = input.read(&mut buffer).map_err(port_io)?;
        if count == 0 {
            break;
        }
        size += count as u64;
        if size > MAX_ARCHIVE {
            return Err(PortError::TooLarge);
        }
        hash.update(&buffer[..count]);
        output.write_all(&buffer[..count]).map_err(port_io)?;
    }
    if hash.finalize().as_bytes() != &package.digest {
        return Err(PortError::InvalidData);
    }
    output.sync_all().map_err(port_io)
}

/// libarchive interprets gzip/PAX/GNU metadata; ustar gives our validator one unambiguous
/// representation. -P preserves absolute names so validation can reject them. Extraction
/// consumes ONLY this validated private copy, never the original archive. Ustar conversion
/// fails closed on unrepresentable names; xattrs, ACLs and file flags are not carried over.
fn normalize_archive(source: &Path, output: &Path) -> Result<(), PortError> {
    let mut child = Command::new("/usr/bin/tar")
        .args(["-cP", "--format=ustar", "-f", "-"])
        .arg(format!("@{}", source.display()))
        .env("COPYFILE_DISABLE", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(port_io)?;
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(output)
            .map_err(port_io)?;
        let size = io::copy(
            &mut child.stdout.take().unwrap().take(MAX_TAR + 1),
            &mut file,
        )
        .map_err(port_io)?;
        if size > MAX_TAR {
            Err(PortError::TooLarge)
        } else {
            Ok(())
        }
    })();
    if result.is_err() {
        let _ = child.kill();
    }
    let status = child.wait().map_err(port_io)?;
    result?;
    if status.success() {
        Ok(())
    } else {
        Err(PortError::InvalidData)
    }
}

#[derive(Clone)]
struct Entry {
    kind: u8,
    target: String,
}

fn field(bytes: &[u8]) -> Result<String, PortError> {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    if bytes[end..].iter().any(|&b| b != 0) {
        return Err(PortError::InvalidData);
    }
    String::from_utf8(bytes[..end].to_vec()).map_err(|_| PortError::InvalidData)
}
fn octal(bytes: &[u8]) -> Result<u64, PortError> {
    let value = std::str::from_utf8(bytes)
        .map_err(|_| PortError::InvalidData)?
        .trim_matches(['\0', ' ']);
    u64::from_str_radix(value, 8).map_err(|_| PortError::InvalidData)
}
fn archive_path(name: &str) -> Result<String, PortError> {
    let path = name.trim_end_matches('/');
    // Portable names also avoid case/Unicode normalization aliases on macOS filesystems.
    if !path.is_ascii()
        || path.bytes().any(|b| b < 32 || b == 127 || b == b'\\')
        || path
            .split('/')
            .any(|p| p.is_empty() || p == "." || p == "..")
        || path.split('/').next() != Some(ROOT)
        || path.split('/').count() > 64
    {
        return Err(PortError::InvalidData);
    }
    Ok(path.to_owned())
}

fn validate_tar(path: &Path, max_size: u64, max_entries: usize) -> Result<(), PortError> {
    let mut file = File::open(path).map_err(port_io)?;
    let mut entries = HashMap::new();
    let mut total = 0_u64;
    loop {
        let mut header = [0_u8; 512];
        file.read_exact(&mut header)
            .map_err(|_| PortError::InvalidData)?;
        if header == [0; 512] {
            // No second archive or nonzero payload may hide beyond the first end marker.
            let mut rest = [0; 8192];
            loop {
                let n = file.read(&mut rest).map_err(port_io)?;
                if n == 0 {
                    break;
                }
                if rest[..n].iter().any(|&b| b != 0) {
                    return Err(PortError::InvalidData);
                }
            }
            break;
        }
        let sum: u64 = header
            .iter()
            .enumerate()
            .map(|(i, &b)| {
                if (148..156).contains(&i) {
                    32
                } else {
                    b as u64
                }
            })
            .sum();
        if octal(&header[148..156])? != sum || &header[257..263] != b"ustar\0" {
            return Err(PortError::InvalidData);
        }
        let name = field(&header[..100])?;
        let prefix = field(&header[345..500])?;
        let path = archive_path(&if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        })?;
        let kind = header[156];
        let size = octal(&header[124..136])?;
        if !matches!(kind, 0 | b'0' | b'1' | b'2' | b'5')
            || octal(&header[100..108])? & 0o6022 != 0
            || (!matches!(kind, 0 | b'0') && size != 0)
            || (path == ROOT && kind != b'5')
        {
            return Err(PortError::InvalidData);
        }
        total = total.checked_add(size).ok_or(PortError::TooLarge)?;
        if total > max_size || entries.len() >= max_entries {
            return Err(PortError::TooLarge);
        }
        let entry = Entry {
            kind,
            target: field(&header[157..257])?,
        };
        if entries.insert(path.to_ascii_lowercase(), entry).is_some() {
            return Err(PortError::InvalidData);
        }
        let padded = size.div_ceil(512) * 512;
        if io::copy(&mut Read::by_ref(&mut file).take(padded), &mut io::sink()).map_err(port_io)?
            != padded
        {
            return Err(PortError::InvalidData);
        }
    }
    if entries.is_empty() {
        return Err(PortError::InvalidData);
    }
    for (path, entry) in &entries {
        let parts: Vec<_> = path.split('/').collect();
        for i in 1..parts.len() {
            if entries
                .get(&parts[..i].join("/"))
                .is_some_and(|e| e.kind != b'5')
            {
                return Err(PortError::InvalidData);
            }
        }
        match entry.kind {
            b'1' => {
                let target = archive_path(&entry.target)?.to_ascii_lowercase();
                if !entries
                    .get(&target)
                    .is_some_and(|e| matches!(e.kind, 0 | b'0'))
                {
                    return Err(PortError::InvalidData);
                }
            }
            b'2' => {
                resolve_link(&parts[..parts.len() - 1], &entry.target, &entries, &mut 0)?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn resolve_link(
    base: &[&str],
    target: &str,
    entries: &HashMap<String, Entry>,
    hops: &mut usize,
) -> Result<Vec<String>, PortError> {
    *hops += 1;
    if *hops > 64 || target.starts_with('/') || !target.is_ascii() || target.contains('\\') {
        return Err(PortError::InvalidData);
    }
    let mut path: Vec<String> = base.iter().map(|p| p.to_string()).collect();
    for component in target.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                if path.len() <= 1 {
                    return Err(PortError::InvalidData);
                }
                path.pop();
            }
            part => {
                path.push(part.to_ascii_lowercase());
                if let Some(entry) = entries.get(&path.join("/")) {
                    if entry.kind == b'2' {
                        path.pop();
                        let base: Vec<_> = path.iter().map(String::as_str).collect();
                        path = resolve_link(&base, &entry.target, entries, hops)?;
                    }
                }
            }
        }
    }
    if path.first().is_none_or(|p| p != "kudzu.app") {
        return Err(PortError::InvalidData);
    }
    Ok(path)
}

fn validate_bundle(bundle: &Path, package: &UpdatePackage) -> Result<(), PortError> {
    let mut bytes = Vec::new();
    File::open(bundle.join("Contents/Info.plist"))
        .map_err(port_io)?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(port_io)?;
    if bytes.len() > 1024 * 1024 {
        return Err(PortError::TooLarge);
    }
    let plist =
        plist::Value::from_reader(io::Cursor::new(bytes)).map_err(|_| PortError::InvalidData)?;
    let dict = plist.as_dictionary().ok_or(PortError::InvalidData)?;
    if dict
        .get("CFBundleIdentifier")
        .and_then(plist::Value::as_string)
        != Some(&package.bundle_id)
        || dict
            .get("CFBundleShortVersionString")
            .and_then(plist::Value::as_string)
            != Some(&package.version)
    {
        return Err(PortError::InvalidData);
    }
    Ok(())
}

fn signature_command(bundle: &Path, bundle_id: &str) -> Command {
    let requirement = format!("=anchor apple generic and identifier \"{bundle_id}\" and certificate leaf[subject.OU] = \"V3HN8HXZYK\" and certificate 1[field.1.2.840.113635.100.6.2.6] exists and certificate leaf[field.1.2.840.113635.100.6.1.13] exists");
    let mut command = Command::new("/usr/bin/codesign");
    command
        .args([
            "--verify",
            "--deep",
            "--strict",
            "--test-requirement",
            &requirement,
        ])
        .arg(bundle);
    command
}
fn verify_signature(bundle: &Path, bundle_id: &str) -> Result<(), PortError> {
    let status = signature_command(bundle, bundle_id)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(port_io)?;
    if status.success() {
        Ok(())
    } else {
        Err(PortError::InvalidData)
    }
}

/// S70 builds the updater archive in xtask. xtask may not depend on this crate, so the test below
/// compiles xtask's own archiving function and feeds its output to the validator in this file.
#[cfg(test)]
#[allow(dead_code)]
#[path = "../../../xtask/src/archive.rs"]
mod release_archive;

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::Cell,
        os::unix::fs::{MetadataExt, PermissionsExt},
    };

    const ID: &str = "sh.saeed.kudzu";
    struct Fixture {
        dir: Scratch,
        bundle: PathBuf,
        package: UpdatePackage,
    }
    impl Fixture {
        fn new() -> Self {
            let dir = Scratch::beside(&std::env::temp_dir()).unwrap();
            let bundle = dir.0.join("Installed Kudzu.app");
            fs::create_dir(&bundle).unwrap();
            fs::write(bundle.join("old"), b"running bundle").unwrap();
            let package = UpdatePackage {
                archive: dir.0.join("fixture.tar.gz"),
                bundle_id: ID.into(),
                version: "2.0.0".into(),
                digest: [0; 32],
            };
            let mut this = Self {
                dir,
                bundle,
                package,
            };
            this.write(good_entries(ID, "2.0.0"));
            this
        }
        fn write(&mut self, entries: Vec<TestEntry>) {
            let mut bytes = Vec::new();
            for entry in entries {
                bytes.extend(entry.encode());
            }
            bytes.extend([0; 1024]);
            fs::write(&self.package.archive, &bytes).unwrap();
            self.package.digest = *blake3::hash(&bytes).as_bytes();
        }
        fn installer(&self) -> BundleInstaller {
            BundleInstaller::new(self.bundle.clone(), ID, "1.0.0").unwrap()
        }
        fn unchanged(&self) {
            assert_eq!(
                fs::read(self.bundle.join("old")).unwrap(),
                b"running bundle"
            );
            assert_eq!(
                fs::read_dir(&self.dir.0).unwrap().count(),
                3,
                "no probe/staging debris"
            );
        }
    }
    struct TestEntry {
        name: String,
        kind: u8,
        mode: u32,
        target: String,
        data: Vec<u8>,
    }
    impl TestEntry {
        fn file(name: &str, data: &[u8]) -> Self {
            Self {
                name: name.into(),
                kind: b'0',
                mode: 0o755,
                target: String::new(),
                data: data.to_vec(),
            }
        }
        fn link(name: &str, target: &str, kind: u8) -> Self {
            Self {
                target: target.into(),
                kind,
                ..Self::file(name, b"")
            }
        }
        fn encode(&self) -> Vec<u8> {
            let mut header = [0_u8; 512];
            header[..self.name.len()].copy_from_slice(self.name.as_bytes());
            header[100..108].copy_from_slice(format!("{:07o}\0", self.mode).as_bytes());
            header[108..116].copy_from_slice(b"0000000\0");
            header[116..124].copy_from_slice(b"0000000\0");
            header[124..136].copy_from_slice(format!("{:011o}\0", self.data.len()).as_bytes());
            header[136..148].copy_from_slice(b"00000000000\0");
            header[148..156].fill(b' ');
            header[156] = self.kind;
            header[157..157 + self.target.len()].copy_from_slice(self.target.as_bytes());
            header[257..265].copy_from_slice(
                b"ustar\0"
                    .iter()
                    .chain(b"00")
                    .copied()
                    .collect::<Vec<_>>()
                    .as_slice(),
            );
            let sum: u64 = header.iter().map(|&b| b as u64).sum();
            header[148..156].copy_from_slice(format!("{sum:06o}\0 ").as_bytes());
            let mut out = header.to_vec();
            out.extend(&self.data);
            out.resize(out.len().div_ceil(512) * 512, 0);
            out
        }
    }
    fn good_entries(id: &str, version: &str) -> Vec<TestEntry> {
        let mut dict = plist::Dictionary::new();
        dict.insert("CFBundleIdentifier".into(), id.into());
        dict.insert("CFBundleShortVersionString".into(), version.into());
        dict.insert("CFBundleVersion".into(), version.into());
        dict.insert("CFBundleExecutable".into(), "Kudzu".into());
        let mut xml = Vec::new();
        plist::Value::Dictionary(dict)
            .to_writer_xml(&mut xml)
            .unwrap();
        vec![
            TestEntry::link(ROOT, "", b'5'),
            TestEntry::link("Kudzu.app/Contents", "", b'5'),
            TestEntry::link("Kudzu.app/Contents/MacOS", "", b'5'),
            TestEntry::file("Kudzu.app/Contents/Info.plist", &xml),
            TestEntry::file("Kudzu.app/Contents/MacOS/Kudzu", b"new executable"),
        ]
    }
    fn accepted(_: &Path, _: &str) -> Result<(), PortError> {
        Ok(())
    }

    #[test]
    fn validated_fixture_extracts_on_same_volume_and_keeps_backup_until_swap_succeeds() {
        let fixture = Fixture::new();
        let renames = Cell::new(0);
        fixture
            .installer()
            .install_with(
                &fixture.package,
                |path, id| {
                    assert_eq!(id, ID);
                    assert_eq!(
                        path.parent().unwrap().parent().unwrap(),
                        fixture.bundle.parent().unwrap()
                    );
                    assert_eq!(
                        fs::metadata(path).unwrap().dev(),
                        fs::metadata(&fixture.bundle).unwrap().dev()
                    );
                    assert_eq!(
                        fs::read(path.join("Contents/MacOS/Kudzu")).unwrap(),
                        b"new executable"
                    );
                    assert!(fixture.bundle.join("old").exists());
                    Ok(())
                },
                |from, to| {
                    assert!(fixture.bundle.join("Contents/MacOS/Kudzu").exists());
                    assert!(from.join("old").exists());
                    assert_eq!(to.file_name().unwrap(), "previous.app");
                    renames.set(renames.get() + 1);
                    fs::rename(from, to)
                },
            )
            .unwrap();
        assert_eq!(renames.get(), 1);
        assert_eq!(
            fs::read(fixture.bundle.join("Contents/MacOS/Kudzu")).unwrap(),
            b"new executable"
        );
        assert!(!fixture.bundle.join("old").exists());
        assert_eq!(fs::read_dir(&fixture.dir.0).unwrap().count(), 3);
    }

    #[test]
    fn failed_swap_rolls_back_and_failed_rollback_preserves_the_backup() {
        for fail_rollback in [false, true] {
            let fixture = Fixture::new();
            let count = Cell::new(0);
            let result = fixture.installer().install_using(
                &fixture.package,
                accepted,
                |_, _| Err(io::Error::from_raw_os_error(swap::ENOTSUP)),
                |from, to| {
                    count.set(count.get() + 1);
                    if count.get() == 2 || (fail_rollback && count.get() == 3) {
                        Err(io::ErrorKind::PermissionDenied.into())
                    } else {
                        fs::rename(from, to)
                    }
                },
            );
            assert!(result.is_err());
            if fail_rollback {
                assert!(!fixture.bundle.exists());
                let stage = fs::read_dir(&fixture.dir.0)
                    .unwrap()
                    .map(|e| e.unwrap().path())
                    .find(|p| {
                        p.file_name()
                            .unwrap()
                            .to_str()
                            .unwrap()
                            .starts_with(".kudzu-update-")
                    })
                    .unwrap();
                assert_eq!(
                    fs::read(stage.join("previous.app/old")).unwrap(),
                    b"running bundle"
                );
            } else {
                fixture.unchanged();
            }
        }
    }

    #[test]
    fn traversal_absolute_second_root_and_unsafe_archive_metadata_are_rejected() {
        let attacks = [
            TestEntry::file("Kudzu.app/../escape", b"bad"),
            TestEntry::file("../escape", b"bad"),
            TestEntry::file("/Kudzu.app/absolute", b"bad"),
            TestEntry::file("second-root", b"bad"),
            TestEntry::link("Kudzu.app/escape", "../../outside", b'2'),
            TestEntry::link("Kudzu.app/hard", "../outside", b'1'),
            TestEntry::link("Kudzu.app/device", "", b'3'),
            TestEntry::link("Kudzu.app/device", "", b'4'),
            TestEntry::link("Kudzu.app/fifo", "", b'6'),
            TestEntry {
                mode: 0o4755,
                ..TestEntry::file("Kudzu.app/setuid", b"bad")
            },
            TestEntry {
                mode: 0o2755,
                ..TestEntry::file("Kudzu.app/setgid", b"bad")
            },
            TestEntry::file("Kudzu.app/Contents/Info.plist", b"duplicate"),
            TestEntry::file("Kudzu.app/contents/info.plist", b"case collision"),
        ];
        for attack in attacks {
            let mut fixture = Fixture::new();
            let attack_name = attack.name.clone();
            let mut entries = good_entries(ID, "2.0.0");
            entries.push(attack);
            fixture.write(entries);
            assert!(
                fixture
                    .installer()
                    .install_with(
                        &fixture.package,
                        |_, _| panic!("validation must reject {attack_name}"),
                        |_, _| panic!("must not swap")
                    )
                    .is_err(),
                "{attack_name}"
            );
            fixture.unchanged();
            assert!(!fixture.dir.0.join("escape").exists());
        }
    }

    #[test]
    fn internal_links_work_but_link_chains_cannot_escape_or_be_written_through() {
        let mut fixture = Fixture::new();
        let mut entries = good_entries(ID, "2.0.0");
        entries.push(TestEntry::link(
            "Kudzu.app/Contents/MacOS/alias",
            "Kudzu",
            b'2',
        ));
        entries.push(TestEntry::link(
            "Kudzu.app/Contents/MacOS/hard",
            "Kudzu.app/Contents/MacOS/Kudzu",
            b'1',
        ));
        fixture.write(entries);
        fixture
            .installer()
            .install_with(&fixture.package, accepted, |a, b| fs::rename(a, b))
            .unwrap();
        assert_eq!(
            fs::read(fixture.bundle.join("Contents/MacOS/alias")).unwrap(),
            b"new executable"
        );
        assert_eq!(
            fs::metadata(fixture.bundle.join("Contents/MacOS/hard"))
                .unwrap()
                .ino(),
            fs::metadata(fixture.bundle.join("Contents/MacOS/Kudzu"))
                .unwrap()
                .ino()
        );
        for additions in [
            vec![
                TestEntry::link("Kudzu.app/Contents/up", "..", b'2'),
                TestEntry::link("Kudzu.app/Contents/link", "up/../outside", b'2'),
            ],
            vec![
                TestEntry::link("Kudzu.app/a", "b", b'2'),
                TestEntry::link("Kudzu.app/b", "a", b'2'),
            ],
            vec![
                TestEntry::link("Kudzu.app/link", "Contents", b'2'),
                TestEntry::file("Kudzu.app/link/write", b"bad"),
            ],
            vec![TestEntry::link(
                "Kudzu.app/link",
                "Contents/Info.plist",
                b'1',
            )], // hardlinks are archive-root-relative
        ] {
            let mut fixture = Fixture::new();
            let mut entries = good_entries(ID, "2.0.0");
            entries.extend(additions);
            fixture.write(entries);
            assert!(fixture
                .installer()
                .install_with(&fixture.package, accepted, |_, _| panic!("unsafe links"))
                .is_err());
            fixture.unchanged();
        }
    }

    #[test]
    fn signed_version_identity_and_digest_are_checked_against_running_version() {
        for (id, embedded, signed) in [
            ("wrong.id", "2.0.0", "2.0.0"),
            (ID, "2.1.0", "2.0.0"),
            (ID, "0.9.0", "0.9.0"),
            (ID, "1.0.0", "1.0.0"),
            (ID, "1.0.0+new", "1.0.0+new"),
        ] {
            let mut fixture = Fixture::new();
            fixture.write(good_entries(id, embedded));
            fixture.package.version = signed.into();
            assert!(fixture
                .installer()
                .install_with(
                    &fixture.package,
                    |_, _| panic!("invalid identity or version"),
                    |_, _| panic!("must not swap")
                )
                .is_err());
            fixture.unchanged();
        }
        let mut fixture = Fixture::new();
        fixture.package.bundle_id = "other.id".into();
        assert!(fixture
            .installer()
            .install_with(&fixture.package, accepted, |_, _| panic!(
                "wrong package identity"
            ))
            .is_err());
        fixture.unchanged();
        fixture.package.bundle_id = ID.into();
        fixture.package.digest[0] ^= 1;
        assert!(fixture
            .installer()
            .install_with(
                &fixture.package,
                |_, _| panic!("digest mismatch"),
                |_, _| panic!("must not swap")
            )
            .is_err());
        fixture.unchanged();
        // Changing the installed bundle's plist cannot change the running-version baseline.
        fixture.package.digest[0] ^= 1;
        fs::create_dir(fixture.bundle.join("Contents")).unwrap();
        fs::write(
            fixture.bundle.join("Contents/Info.plist"),
            b"disk app differs from running executable",
        )
        .unwrap();
        fixture
            .installer()
            .install_with(&fixture.package, accepted, |a, b| fs::rename(a, b))
            .unwrap();
    }

    #[test]
    fn release_candidates_install_only_when_strictly_higher() {
        for (running, candidate, installs) in [
            ("0.7.0", "0.7.1-rc.1", true),
            ("0.7.1-rc.1", "0.7.1-rc.2", true),
            ("0.7.1-rc.2", "0.7.1", true),
            ("0.7.1-rc.2", "0.7.1-rc.1", false),
            ("0.7.1-rc.1", "0.7.1-rc.1", false),
            ("0.7.1", "0.7.1-rc.9", false),
        ] {
            let mut fixture = Fixture::new();
            fixture.write(good_entries(ID, candidate));
            fixture.package.version = candidate.into();
            let installer = BundleInstaller::new(fixture.bundle.clone(), ID, running).unwrap();
            let result =
                installer.install_with(&fixture.package, accepted, |a, b| fs::rename(a, b));
            assert_eq!(result.is_ok(), installs, "{running} -> {candidate}");
        }
    }

    #[test]
    fn unpacked_bytes_and_entry_count_are_capped_even_without_a_large_archive() {
        let fixture = Fixture::new();
        assert_eq!(
            validate_tar(&fixture.package.archive, 2, MAX_ENTRIES),
            Err(PortError::TooLarge)
        );
        assert_eq!(
            validate_tar(&fixture.package.archive, MAX_EXTRACTED, 2),
            Err(PortError::TooLarge)
        );
        let mut oversized = TestEntry::file("Kudzu.app/huge", b"").encode();
        oversized[124..136].copy_from_slice(format!("{:011o}\0", MAX_EXTRACTED + 1).as_bytes());
        oversized[148..156].fill(b' ');
        let sum: u64 = oversized.iter().map(|&b| b as u64).sum();
        oversized[148..156].copy_from_slice(format!("{sum:06o}\0 ").as_bytes());
        fs::write(&fixture.package.archive, oversized).unwrap();
        assert_eq!(
            validate_tar(&fixture.package.archive, MAX_EXTRACTED, MAX_ENTRIES),
            Err(PortError::TooLarge)
        );
    }

    #[test]
    fn translocation_read_only_and_probes_leave_no_debris() {
        let fixture = Fixture::new();
        assert!(!fixture.installer().probe().unwrap().needs_move);
        fixture.unchanged();
        let translocated = fixture.dir.0.join("AppTranslocation/random/Kudzu.app");
        fs::create_dir_all(&translocated).unwrap();
        assert!(
            BundleInstaller::new(translocated, ID, "1.0.0")
                .unwrap()
                .probe()
                .unwrap()
                .needs_move
        );
        fs::remove_dir_all(fixture.dir.0.join("AppTranslocation")).unwrap();
        let permissions = fs::metadata(&fixture.dir.0).unwrap().permissions();
        fs::set_permissions(&fixture.dir.0, fs::Permissions::from_mode(0o555)).unwrap();
        let result = fixture.installer().probe();
        fs::set_permissions(&fixture.dir.0, permissions).unwrap();
        assert!(result.unwrap().needs_move);
        fixture.unchanged();
        assert!(
            BundleInstaller::new(PathBuf::from("/Volumes/Fixture/Kudzu.app"), ID, "1.0.0")
                .unwrap()
                .probe()
                .unwrap()
                .needs_move
        );
    }

    /// Entry names and type flags of an uncompressed ustar/pax stream, without libarchive's
    /// AppleDouble merging (`tar -t` hides `._` entries).
    fn raw_entries(tar: &[u8]) -> Vec<(String, u8)> {
        let mut at = 0;
        let mut out = Vec::new();
        while at + 512 <= tar.len() && tar[at..at + 512] != [0; 512] {
            let header = &tar[at..at + 512];
            let text = |range: std::ops::Range<usize>| {
                let bytes = &header[range];
                let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
                String::from_utf8_lossy(&bytes[..end]).into_owned()
            };
            let (name, prefix) = (text(0..100), text(345..500));
            out.push((
                if prefix.is_empty() {
                    name
                } else {
                    format!("{prefix}/{name}")
                },
                header[156],
            ));
            let size = u64::from_str_radix(text(124..135).trim(), 8).unwrap() as usize;
            at += 512 + size.div_ceil(512) * 512;
        }
        out
    }
    const XATTR: &str = "com.kudzu.s70-test";
    fn no_apple_metadata(root: &Path) {
        for entry in fs::read_dir(root).unwrap() {
            let path = entry.unwrap().path();
            assert!(
                !path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("._"),
                "AppleDouble file extracted: {}",
                path.display()
            );
            // macOS adds its own `com.apple.provenance` to new files; only the planted one counts.
            let planted = Command::new("/usr/bin/xattr")
                .args(["-p", XATTR])
                .arg(&path)
                .output()
                .unwrap();
            assert!(
                !planted.status.success(),
                "xattr extracted onto {}",
                path.display()
            );
            if path.is_dir() {
                no_apple_metadata(&path);
            }
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn the_archive_made_by_the_release_pipeline_passes_the_validator_and_extracts_clean() {
        let mut fixture = Fixture::new();
        let source = fixture.dir.0.join("release-source");
        let app = source.join(ROOT);
        fs::create_dir_all(app.join("Contents/MacOS")).unwrap();
        fs::write(
            app.join("Contents/Info.plist"),
            &good_entries(ID, "2.0.0")[3].data,
        )
        .unwrap();
        fs::write(app.join("Contents/MacOS/Kudzu"), b"new executable").unwrap();
        // Extended attributes (as `com.apple.provenance` after a copy) are what macOS tar turns
        // into AppleDouble entries.
        for path in [
            app.clone(),
            app.join("Contents"),
            app.join("Contents/Info.plist"),
            app.join("Contents/MacOS/Kudzu"),
        ] {
            checked(
                Command::new("/usr/bin/xattr")
                    .args(["-w", XATTR, "value"])
                    .arg(&path),
            );
        }
        release_archive::create(&source, ROOT, &fixture.package.archive).unwrap();
        let bytes = fs::read(&fixture.package.archive).unwrap();
        fixture.package.digest = *blake3::hash(&bytes).as_bytes();

        let tar = Command::new("/usr/bin/gzip")
            .arg("-dc")
            .arg(&fixture.package.archive)
            .output()
            .unwrap();
        assert!(tar.status.success());
        let entries = raw_entries(&tar.stdout);
        assert!(entries.len() >= 4, "{entries:?}");
        for (name, kind) in &entries {
            assert!(
                name.split('/').all(|part| !part.starts_with("._")),
                "AppleDouble entry {name}"
            );
            assert!(matches!(kind, 0 | b'0' | b'5'), "{name}: type {kind}");
        }
        assert!(!tar.stdout.windows(5).any(|w| w == b"xattr"));

        let normalized = fixture.dir.0.join("normalized.tar");
        normalize_archive(&fixture.package.archive, &normalized).unwrap();
        validate_tar(&normalized, MAX_EXTRACTED, MAX_ENTRIES).unwrap();
        fixture
            .installer()
            .install_with(
                &fixture.package,
                |extracted, _| {
                    no_apple_metadata(extracted);
                    assert_eq!(
                        fs::read(extracted.join("Contents/MacOS/Kudzu")).unwrap(),
                        b"new executable"
                    );
                    Ok(())
                },
                |from, to| fs::rename(from, to),
            )
            .unwrap();
        assert_eq!(
            fs::read(fixture.bundle.join("Contents/MacOS/Kudzu")).unwrap(),
            b"new executable"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn ad_hoc_code_signature_passes_integrity_but_cannot_pass_the_pinned_requirement() {
        let fixture = Fixture::new();
        let source = fixture.dir.0.join(ROOT);
        fs::create_dir_all(source.join("Contents/MacOS")).unwrap();
        let entries = good_entries(ID, "2.0.0");
        fs::write(source.join("Contents/Info.plist"), &entries[3].data).unwrap();
        fs::copy("/usr/bin/true", source.join("Contents/MacOS/Kudzu")).unwrap();
        assert!(Command::new("/usr/bin/codesign")
            .args(["--force", "--sign", "-", "--timestamp=none"])
            .arg(&source)
            .status()
            .unwrap()
            .success());
        assert!(Command::new("/usr/bin/codesign")
            .args(["--verify", "--deep", "--strict"])
            .arg(&source)
            .status()
            .unwrap()
            .success());
        assert_eq!(verify_signature(&source, ID), Err(PortError::InvalidData));
        let command = signature_command(&source, ID);
        let args: Vec<_> = command.get_args().collect();
        assert_eq!(
            &args[..4],
            &["--verify", "--deep", "--strict", "--test-requirement"]
        );
        let requirement = args[4].to_str().unwrap();
        assert!(requirement.contains("V3HN8HXZYK"));
        assert!(requirement.contains("identifier \"sh.saeed.kudzu\""));
        assert!(requirement.contains("anchor apple generic"));
    }

    // Test-only certificate and key in a disposable keychain; no real identity is selected.
    struct TestKeychain(PathBuf);
    impl Drop for TestKeychain {
        fn drop(&mut self) {
            let _ = Command::new("/usr/bin/security")
                .arg("delete-keychain")
                .arg(&self.0)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }
    fn checked(command: &mut Command) {
        let output = command.stdin(Stdio::null()).output().unwrap();
        assert!(
            output.status.success(),
            "fixture command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    fn keychain_setting(command: &str) -> Vec<u8> {
        Command::new("/usr/bin/security")
            .args([command, "-d", "user"])
            .output()
            .unwrap()
            .stdout
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_self_signed_fixture_with_the_expected_id_and_team_is_rejected() {
        let mut fixture = Fixture::new();
        let search_list = keychain_setting("list-keychains");
        let default = keychain_setting("default-keychain");
        let keychain = TestKeychain(fixture.dir.0.join("fixture.keychain-db"));
        checked(
            Command::new("/usr/bin/security")
                .args(["create-keychain", "-p", "fixture"])
                .arg(&keychain.0),
        );
        assert_eq!(keychain_setting("list-keychains"), search_list);
        let config = fixture.dir.0.join("cert.conf");
        fs::write(&config, "[req]\ndistinguished_name=dn\nx509_extensions=ext\nprompt=no\n[dn]\nCN=Kudzu S36 Test Fixture\nOU=V3HN8HXZYK\n[ext]\nbasicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature\nextendedKeyUsage=critical,codeSigning\n").unwrap();
        let key = fixture.dir.0.join("key.pem");
        let cert = fixture.dir.0.join("cert.pem");
        let p12 = fixture.dir.0.join("fixture.p12");
        checked(
            Command::new("/usr/bin/openssl")
                .args([
                    "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1", "-keyout",
                ])
                .arg(&key)
                .arg("-out")
                .arg(&cert)
                .arg("-config")
                .arg(&config),
        );
        checked(
            Command::new("/usr/bin/openssl")
                .args(["pkcs12", "-export", "-out"])
                .arg(&p12)
                .arg("-inkey")
                .arg(&key)
                .arg("-in")
                .arg(&cert)
                .args(["-passout", "pass:fixture"]),
        );
        checked(
            Command::new("/usr/bin/security")
                .arg("import")
                .arg(&p12)
                .arg("-k")
                .arg(&keychain.0)
                .args(["-P", "fixture", "-T", "/usr/bin/codesign"]),
        );
        checked(
            Command::new("/usr/bin/security")
                .args([
                    "set-key-partition-list",
                    "-S",
                    "apple-tool:,apple:,codesign:",
                    "-s",
                    "-k",
                    "fixture",
                ])
                .arg(&keychain.0),
        );
        let source = fixture.dir.0.join(ROOT);
        fs::create_dir_all(source.join("Contents/MacOS")).unwrap();
        fs::write(
            source.join("Contents/Info.plist"),
            &good_entries(ID, "2.0.0")[3].data,
        )
        .unwrap();
        fs::copy("/usr/bin/true", source.join("Contents/MacOS/Kudzu")).unwrap();
        checked(
            Command::new("/usr/bin/codesign")
                .args(["--force", "--sign", "Kudzu S36 Test Fixture", "--keychain"])
                .arg(&keychain.0)
                .arg("--timestamp=none")
                .arg(&source),
        );
        checked(
            Command::new("/usr/bin/codesign")
                .args(["--verify", "--deep", "--strict"])
                .arg(&source),
        );
        // Assert an actual requirement failure, rather than an accidentally invalid argument.
        let rejected = signature_command(&source, ID).output().unwrap();
        assert!(!rejected.status.success());
        let error = String::from_utf8_lossy(&rejected.stderr);
        assert!(error.contains("failed to satisfy"), "{error}");
        assert!(!error.contains("invalid requirement"));
        checked(
            Command::new("/usr/bin/tar")
                .args(["-czf"])
                .arg(&fixture.package.archive)
                .args(["--format=ustar", "-C"])
                .arg(&fixture.dir.0)
                .arg(ROOT)
                .env("COPYFILE_DISABLE", "1"),
        );
        fixture.package.digest =
            *blake3::hash(&fs::read(&fixture.package.archive).unwrap()).as_bytes();
        assert_eq!(
            fixture.installer().install(&fixture.package),
            Err(PortError::InvalidData)
        );
        assert!(fixture.bundle.join("old").exists());
        assert!(!fixture.bundle.join("Contents").exists());
        drop(keychain);
        assert_eq!(keychain_setting("list-keychains"), search_list);
        assert_eq!(keychain_setting("default-keychain"), default);
    }

    #[test]
    fn review_swap_failure_between_steps_never_removes_the_installed_path() {
        let fixture = Fixture::new();
        let result = fixture
            .installer()
            .install_with(&fixture.package, accepted, |from, to| {
                assert!(
                    fixture.bundle.is_dir(),
                    "installed path must always hold a whole app"
                );
                assert!(
                    fixture.bundle.join("Contents/MacOS/Kudzu").exists(),
                    "atomic exchange happens before backup retention"
                );
                assert!(from.join("old").exists());
                assert_eq!(to.file_name().unwrap(), "previous.app");
                Err(io::ErrorKind::PermissionDenied.into())
            });
        assert!(result.is_err());
        fixture.unchanged();
    }

    #[test]
    fn review_equal_or_newer_disk_version_skips_without_verifying_or_swapping() {
        for version in ["2.0.0", "2.0.0+local", "3.0.0"] {
            let fixture = Fixture::new();
            let stale = fixture.dir.0.join(".kudzu-update-abandoned");
            fs::create_dir(&stale).unwrap();
            fs::create_dir(fixture.bundle.join("Contents")).unwrap();
            fs::write(
                fixture.bundle.join("Contents/Info.plist"),
                &good_entries(ID, version)[3].data,
            )
            .unwrap();
            fixture
                .installer()
                .install_with(
                    &fixture.package,
                    |_, _| panic!("must skip"),
                    |_, _| panic!("must skip"),
                )
                .unwrap();
            assert!(!stale.exists(), "install entry also sweeps stale folders");
            fixture.unchanged();
        }
    }

    #[test]
    fn review_group_and_world_writable_modes_are_rejected() {
        for mode in [0o775, 0o757, 0o777] {
            let mut fixture = Fixture::new();
            let mut entries = good_entries(ID, "2.0.0");
            entries.push(TestEntry {
                mode,
                ..TestEntry::file("Kudzu.app/writable", b"unsafe")
            });
            fixture.write(entries);
            assert_eq!(
                validate_tar(&fixture.package.archive, MAX_EXTRACTED, MAX_ENTRIES),
                Err(PortError::InvalidData)
            );
        }
    }

    #[test]
    fn review_killing_the_installer_between_exchange_and_backup_keeps_a_whole_app() {
        use std::{
            io::{BufRead, BufReader},
            process::Stdio,
        };
        let fixture = Fixture::new();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "update_install::tests::review_swap_crash_child",
                "--ignored",
                "--nocapture",
            ])
            .env("KUDZU_SWAP_TEST_BUNDLE", &fixture.bundle)
            .env("KUDZU_SWAP_TEST_ARCHIVE", &fixture.package.archive)
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut output = BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        loop {
            assert_ne!(output.read_line(&mut line).unwrap(), 0);
            if line.contains("EXCHANGED") {
                break;
            }
            line.clear();
        }
        assert_eq!(
            fs::read(fixture.bundle.join("Contents/MacOS/Kudzu")).unwrap(),
            b"new executable"
        );
        let stage = fs::read_dir(&fixture.dir.0)
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| {
                p.is_dir()
                    && p.file_name()
                        .unwrap()
                        .to_string_lossy()
                        .starts_with(".kudzu-update-")
            })
            .unwrap();
        assert!(stage.join("Kudzu.app/old").exists());
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(!fixture.installer().probe().unwrap().needs_move);
        assert!(
            !stage.exists(),
            "crash lease released and debris swept by probe"
        );
        assert_eq!(
            fs::read(fixture.bundle.join("Contents/MacOS/Kudzu")).unwrap(),
            b"new executable"
        );
    }

    #[test]
    #[ignore = "only invoked by the isolated atomic-swap crash parent"]
    fn review_swap_crash_child() {
        let bundle =
            PathBuf::from(std::env::var_os("KUDZU_SWAP_TEST_BUNDLE").expect("isolated child only"));
        let archive = PathBuf::from(
            std::env::var_os("KUDZU_SWAP_TEST_ARCHIVE").expect("isolated child only"),
        );
        assert!(bundle.starts_with(std::env::temp_dir()));
        assert_eq!(archive.parent(), bundle.parent());
        let package = UpdatePackage {
            digest: *blake3::hash(&fs::read(&archive).unwrap()).as_bytes(),
            archive,
            bundle_id: ID.into(),
            version: "2.0.0".into(),
        };
        BundleInstaller::new(bundle, ID, "1.0.0")
            .unwrap()
            .install_with(&package, accepted, |_, _| {
                println!("EXCHANGED");
                std::io::stdout().flush().unwrap();
                loop {
                    std::thread::park();
                }
            })
            .unwrap();
        panic!("parent must kill this process");
    }

    #[test]
    fn review_failed_atomic_rollback_still_keeps_the_published_app_and_old_copy() {
        let fixture = Fixture::new();
        let swaps = Cell::new(0);
        let result = fixture.installer().install_using(
            &fixture.package,
            accepted,
            |from, to| {
                swaps.set(swaps.get() + 1);
                if swaps.get() == 1 {
                    swap::exchange(from, to)
                } else {
                    Err(io::ErrorKind::PermissionDenied.into())
                }
            },
            |_, _| Err(io::ErrorKind::PermissionDenied.into()),
        );
        assert!(result.is_err());
        assert_eq!(swaps.get(), 2);
        assert_eq!(
            fs::read(fixture.bundle.join("Contents/MacOS/Kudzu")).unwrap(),
            b"new executable"
        );
        let stage = fs::read_dir(&fixture.dir.0)
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| {
                p.is_dir()
                    && p.file_name()
                        .unwrap()
                        .to_string_lossy()
                        .starts_with(".kudzu-update-")
            })
            .unwrap();
        assert_eq!(
            fs::read(stage.join("Kudzu.app/old")).unwrap(),
            b"running bundle"
        );
    }

    #[test]
    fn review_disk_version_is_rechecked_after_verification_before_publication() {
        let fixture = Fixture::new();
        fixture
            .installer()
            .install_using(
                &fixture.package,
                |_, _| {
                    fs::create_dir(fixture.bundle.join("Contents")).unwrap();
                    fs::write(
                        fixture.bundle.join("Contents/Info.plist"),
                        &good_entries(ID, "3.0.0")[3].data,
                    )
                    .unwrap();
                    Ok(())
                },
                |_, _| panic!("newer app must not be swapped"),
                |_, _| panic!("newer app must not be renamed"),
            )
            .unwrap();
        fixture.unchanged();
    }
}
