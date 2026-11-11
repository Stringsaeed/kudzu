use super::*;

#[test]
fn natural_sort_is_case_insensitive_and_numeric() {
    let mut names = vec!["b.yml", "A10.yml", "a2.yml", "Work", "base.yml"];
    names.sort_by(|a, b| natural_cmp(a, b));
    assert_eq!(
        names,
        vec!["a2.yml", "A10.yml", "b.yml", "base.yml", "Work"]
    );
}

#[test]
fn normalize_and_common_parent() {
    assert_eq!(
        normalize(Path::new("/a/b/./../c/d.yml")),
        PathBuf::from("/a/c/d.yml")
    );
    let files = [
        PathBuf::from("/x/m/base.yml"),
        PathBuf::from("/x/m/work/a.yml"),
    ];
    assert_eq!(common_parent(&files), Some(PathBuf::from("/x/m")));
}
