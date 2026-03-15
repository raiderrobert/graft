use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn graft_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_graft"))
}

#[test]
fn test_init_creates_manifest() {
    let dir = TempDir::new().unwrap();
    let output = graft_bin()
        .arg("init")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout: {}, stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(dir.path().join("graft.toml").exists());
    let content = fs::read_to_string(dir.path().join("graft.toml")).unwrap();
    assert!(content.contains("# Graft"));
}

#[test]
fn test_init_idempotent() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("graft.toml"), "# existing").unwrap();
    let output = graft_bin()
        .arg("init")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    let content = fs::read_to_string(dir.path().join("graft.toml")).unwrap();
    assert_eq!(content, "# existing");
}
