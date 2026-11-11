use super::*;
use serde_json::json;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let dir = std::env::temp_dir()
            .join(format!("kudzu-deps-{}", uuid()))
            .join("workspace");
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname='kudzu-app'\nversion='0.1.0'\nedition='2021'\n",
        )
        .unwrap();
        fs::write(dir.join("src/lib.rs"), "").unwrap();
        Self(dir.canonicalize().unwrap())
    }
    fn metadata(&self) -> Value {
        let output = Command::new("cargo")
            .args(["metadata", "--offline", "--format-version", "1"])
            .current_dir(&self.0)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }
    fn check(&self) -> Result<()> {
        validate(&self.0, &self.metadata())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(self.0.parent().unwrap()).unwrap();
    }
}
fn uuid() -> String {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    format!(
        "{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

#[test]
fn clean_workspace_passes() {
    assert!(Fixture::new().check().is_ok());
}

#[test]
fn forbidden_edge_fails() {
    let f = Fixture::new();
    fs::create_dir_all(f.0.join("engine/src")).unwrap();
    fs::write(
        f.0.join("engine/Cargo.toml"),
        "[package]\nname='kudzu-engine'\nversion='0.1.0'\nedition='2021'\n",
    )
    .unwrap();
    fs::write(f.0.join("engine/src/lib.rs"), "").unwrap();
    let p = f.0.join("Cargo.toml");
    let mut s = fs::read_to_string(&p).unwrap();
    s.push_str("\n[dependencies]\nkudzu-engine={path='engine'}\n");
    fs::write(p, s).unwrap();
    assert!(f.check().unwrap_err().contains("forbidden edge"));
}

#[test]
fn external_path_dependency_fails() {
    let f = Fixture::new();
    let external = f.0.parent().unwrap().join("desktop");
    fs::create_dir_all(external.join("src")).unwrap();
    fs::write(
        external.join("Cargo.toml"),
        "[package]\nname='external'\nversion='0.1.0'\nedition='2021'\n",
    )
    .unwrap();
    fs::write(external.join("src/lib.rs"), "").unwrap();
    let p = f.0.join("Cargo.toml");
    let mut s = fs::read_to_string(&p).unwrap();
    s.push_str(concat!(
        "\n[dependencies]\nexternal={path='..",
        "/desktop'}\n"
    ));
    fs::write(p, s).unwrap();
    assert!(f.check().unwrap_err().contains("escapes workspace"));
}

#[test]
fn external_path_attribute_and_includes_fail() {
    for expr in [
        "#[path = PATH] mod external;",
        "const X: &str = include_str!(PATH);",
        "const X: &[u8] = include_bytes!(PATH);",
        "include!(PATH);",
    ] {
        let f = Fixture::new();
        let external = Fixture::new();
        let expr = expr.replace("PATH", &format!("{:?}", external.0.join("src/lib.rs")));
        fs::write(f.0.join("src/lib.rs"), expr).unwrap();
        assert!(f.check().unwrap_err().contains("escapes workspace"));
    }
}

#[test]
fn build_script_fails() {
    let f = Fixture::new();
    fs::write(f.0.join("build.rs"), "fn main() {}").unwrap();
    assert!(f.check().unwrap_err().contains("build script"));
}

#[test]
fn transitive_forbidden_crate_fails() {
    let f = Fixture::new();
    let mut m = f.metadata();
    let id = m["workspace_members"][0].clone();
    m["packages"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"tokio","name":"tokio","source":"registry"}));
    m["resolve"]["nodes"][0]["dependencies"] = json!(["tokio"]);
    m["resolve"]["nodes"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"tokio","dependencies":[]}));
    assert!(validate(&f.0, &m)
        .unwrap_err()
        .contains("forbidden dependency tree"));
    assert_ne!(id, Value::Null);
}

#[test]
fn raw_paths_and_computed_paths_cannot_bypass_check() {
    let f = Fixture::new();
    assert!(source_paths(
        &f.0,
        &f.0.join("src/lib.rs"),
        "include_str!(r#\"/etc/passwd\"#)"
    )
    .is_err());
    assert!(source_paths(
        &f.0,
        &f.0.join("src/lib.rs"),
        "include_str!(concat!(\"x\", \"y\"))"
    )
    .is_err());
}

#[test]
fn raw_identifiers_are_not_strings_and_do_not_bypass_path_checks() {
    let f = Fixture::new();
    let file = f.0.join("src/lib.rs");
    assert!(source_paths(&f.0, &file, "event.r#type();").is_ok());
    for source in [
        "r#include_str!(\"/etc/passwd\")",
        "r#include_bytes!(r#\"/etc/passwd\"#)",
        "#[r#path = \"/etc/passwd\"] mod external;",
    ] {
        assert!(source_paths(&f.0, &file, source)
            .unwrap_err()
            .contains("escapes workspace"));
    }
}

#[test]
fn unused_manifest_path_dependency_fails() {
    let f = Fixture::new();
    let external = Fixture::new();
    let p = f.0.join("Cargo.toml");
    let mut s = fs::read_to_string(&p).unwrap();
    s.push_str(&format!(
        "\n[workspace.dependencies]\nexternal={{path={:?}}}\n",
        external.0
    ));
    fs::write(p, s).unwrap();
    assert!(f.check().unwrap_err().contains("escapes workspace"));
}

fn add_path_crate(f: &Fixture, dir: &str, name: &str, deps: &str) {
    fs::create_dir_all(f.0.join(dir).join("src")).unwrap();
    fs::write(
        f.0.join(dir).join("Cargo.toml"),
        format!("[package]\nname='{name}'\nversion='0.1.0'\nedition='2021'\n{deps}"),
    )
    .unwrap();
    fs::write(f.0.join(dir).join("src/lib.rs"), "").unwrap();
    let p = f.0.join("Cargo.toml");
    let mut s = fs::read_to_string(&p).unwrap();
    if !s.contains("[dependencies]") {
        s.push_str("\n[dependencies]\n");
    }
    s.push_str(&format!("{name}={{path='{dir}'}}\n"));
    fs::write(p, s).unwrap();
}

#[test]
fn unknown_first_party_crate_fails() {
    let f = Fixture::new();
    add_path_crate(&f, "bench", "kudzu-bench", "");
    assert!(f.check().unwrap_err().contains("unknown first-party crate"));
}

#[test]
fn spike_may_compose_layers() {
    let f = Fixture::new();
    add_path_crate(&f, "core", "kudzu-core", "");
    add_path_crate(
        &f,
        "spikes/editor",
        "spike-editor",
        "[dependencies]\nkudzu-core={path='../../core'}\n",
    );
    assert!(f.check().is_ok(), "{:?}", f.check());
}

#[test]
fn non_rust_path_and_include_targets_fail() {
    for expr in ["#[path = \"x.txt\"] mod x;", "include!(\"x.txt\");"] {
        let f = Fixture::new();
        fs::write(f.0.join("src/x.txt"), "").unwrap();
        fs::write(f.0.join("src/lib.rs"), expr).unwrap();
        assert!(f.check().unwrap_err().contains("must name a .rs file"));
    }
}

#[test]
fn data_includes_may_name_any_file() {
    let f = Fixture::new();
    fs::write(f.0.join("src/x.txt"), "").unwrap();
    fs::write(
        f.0.join("src/lib.rs"),
        "const X: &str = include_str!(\"x.txt\");",
    )
    .unwrap();
    assert!(f.check().is_ok());
}

#[test]
fn frozen_app_mentions_fail_and_this_workspace_passes() {
    let frozen = concat!("apps/", "desktop");
    let parent = concat!("../", "desktop");
    for bad in [
        format!("{frozen}/README.md"),
        frozen.to_string(),
        format!("{parent}/src"),
    ] {
        let f = Fixture::new();
        fs::write(f.0.join("src/lib.rs"), format!("const P: &str = {bad:?};")).unwrap();
        assert!(f.check().unwrap_err().contains("frozen app"), "{bad}");
    }
    let f = Fixture::new();
    fs::write(
        f.0.join("src/lib.rs"),
        format!("const P: &str = \"{frozen}-v2/README.md\";"),
    )
    .unwrap();
    assert!(f.check().is_ok());
}
