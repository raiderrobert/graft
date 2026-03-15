use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn graft_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_graft"))
}

#[test]
fn test_remove_keeps_local_file() {
    let dir = TempDir::new().unwrap();
    // Create graft.toml with a dep
    fs::write(
        dir.path().join("graft.toml"),
        r#"
[grafts.myfile]
source = "gh:owner/repo/file"
version = "v1.0.0"
dest = "myfile.txt"
"#,
    )
    .unwrap();
    // Create lockfile
    fs::write(
        dir.path().join("graft.lock"),
        r#"
[grafts.myfile]
source = "gh:owner/repo/file"
version = "v1.0.0"
commit = "abc123"
checksum = "sha256:000"
"#,
    )
    .unwrap();
    // Create the actual file
    fs::write(dir.path().join("myfile.txt"), "content").unwrap();

    let output = graft_bin()
        .args(["remove", "myfile"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // File should still exist
    assert!(dir.path().join("myfile.txt").exists());
    // But graft.toml should not contain myfile
    let manifest = fs::read_to_string(dir.path().join("graft.toml")).unwrap();
    assert!(!manifest.contains("myfile"));
}

#[test]
fn test_remove_nonexistent() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("graft.toml"), "").unwrap();
    let output = graft_bin()
        .args(["remove", "nonexistent"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(!output.status.success());
}
