use std::fs;
use std::process::Command;

use sha2::{Digest, Sha256};
use tempfile::TempDir;

fn graft_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_graft"))
}

fn sha256_checksum(content: &[u8]) -> String {
    let hash = hex::encode(Sha256::digest(content));
    format!("sha256:{}", hash)
}

fn write_manifest(dir: &TempDir, name: &str, dest: &str) {
    fs::write(
        dir.path().join("graft.toml"),
        format!(
            r#"[grafts.{name}]
source = "gh:owner/repo/{dest}"
version = "v1.0.0"
dest = "{dest}"
"#
        ),
    )
    .unwrap();
}

fn write_lockfile(dir: &TempDir, name: &str, dest: &str, checksum: &str) {
    fs::write(
        dir.path().join("graft.lock"),
        format!(
            r#"[grafts.{name}]
source = "gh:owner/repo/{dest}"
version = "v1.0.0"
commit = "abc123def456abc123def456abc123def456abcd"
checksum = "{checksum}"
"#
        ),
    )
    .unwrap();
}

#[test]
fn adopt_shows_modified_when_local_differs() {
    let dir = TempDir::new().unwrap();

    let upstream_content = b"upstream content";
    let checksum = sha256_checksum(upstream_content);

    write_manifest(&dir, "config", "config.yml");
    write_lockfile(&dir, "config", "config.yml", &checksum);

    // Local file has DIFFERENT content than upstream
    fs::write(dir.path().join("config.yml"), "my local modifications").unwrap();

    let output = graft_bin()
        .arg("list")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("modified"),
        "Expected 'modified' in output: {}",
        stdout
    );
}

#[test]
fn adopt_shows_synced_when_checksum_matches() {
    let dir = TempDir::new().unwrap();

    let content = b"exact upstream content";
    let checksum = sha256_checksum(content);

    write_manifest(&dir, "config", "config.yml");
    write_lockfile(&dir, "config", "config.yml", &checksum);

    // Local file has SAME content as what lockfile expects
    fs::write(dir.path().join("config.yml"), content).unwrap();

    let output = graft_bin()
        .arg("list")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("synced"),
        "Expected 'synced' in output: {}",
        stdout
    );
}

#[test]
fn check_fails_when_adopted_file_modified() {
    let dir = TempDir::new().unwrap();

    let upstream_content = b"original upstream";
    let checksum = sha256_checksum(upstream_content);

    write_manifest(&dir, "config", "config.yml");
    write_lockfile(&dir, "config", "config.yml", &checksum);

    // Local file differs from upstream checksum
    fs::write(dir.path().join("config.yml"), "different content").unwrap();

    let output = graft_bin()
        .arg("check")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "Expected check to fail for modified graft"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("modified"),
        "Expected 'modified' in output: {}",
        stdout
    );
}

#[test]
fn check_passes_when_adopted_file_matches() {
    let dir = TempDir::new().unwrap();

    let content = b"matching content";
    let checksum = sha256_checksum(content);

    write_manifest(&dir, "config", "config.yml");
    write_lockfile(&dir, "config", "config.yml", &checksum);

    fs::write(dir.path().join("config.yml"), content).unwrap();

    let output = graft_bin()
        .arg("check")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "Expected check to pass: stdout={}, stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("in sync"),
        "Expected 'in sync' in output: {}",
        stdout
    );
}

#[test]
fn list_shows_missing_when_file_absent() {
    let dir = TempDir::new().unwrap();

    let checksum = sha256_checksum(b"some content");

    write_manifest(&dir, "config", "config.yml");
    write_lockfile(&dir, "config", "config.yml", &checksum);

    // Do NOT create the local file

    let output = graft_bin()
        .arg("list")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("missing"),
        "Expected 'missing' in output: {}",
        stdout
    );
}
