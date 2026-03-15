use std::path::PathBuf;

use cucumber::{given, then, when, World};
use tempfile::TempDir;

#[derive(Debug, Default, World)]
pub struct GraftWorld {
    /// Temp directory for test isolation (held to prevent cleanup)
    _temp_dir: Option<TempDir>,
    /// Working directory path
    pub work_dir: Option<PathBuf>,
    /// Last command's exit code
    pub exit_code: Option<i32>,
    /// Last command's stdout
    pub stdout: String,
    /// Last command's stderr
    pub stderr: String,
}

fn graft_binary() -> PathBuf {
    let mut path = std::env::current_exe().unwrap();
    // test binary is in target/debug/deps/, graft binary is in target/debug/
    path.pop(); // remove binary name
    if path.ends_with("deps") {
        path.pop(); // remove deps/
    }
    path.push("graft");
    path
}

// --- Given steps ---

#[given("an empty project directory")]
fn empty_project_directory(world: &mut GraftWorld) {
    let dir = TempDir::new().unwrap();
    world.work_dir = Some(dir.path().to_path_buf());
    world._temp_dir = Some(dir);
}

#[given(expr = "a file {string} with content {string}")]
fn file_with_content(world: &mut GraftWorld, filename: String, content: String) {
    let dir = world.work_dir.as_ref().expect("work_dir not set");
    let path = dir.join(&filename);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&path, &content).unwrap();
}

#[given(expr = "an empty project directory with {string}")]
fn empty_project_directory_with_file(world: &mut GraftWorld, filename: String) {
    empty_project_directory(world);
    let dir = world.work_dir.as_ref().expect("work_dir not set");
    let path = dir.join(&filename);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&path, "").unwrap();
}

// --- When steps ---

#[when(expr = "I run {string}")]
fn run_command(world: &mut GraftWorld, command: String) {
    let dir = world.work_dir.as_ref().expect("work_dir not set");
    let parts: Vec<&str> = command.split_whitespace().collect();
    assert!(!parts.is_empty(), "empty command string");

    // First part should be "graft", use our built binary
    assert_eq!(parts[0], "graft", "only graft commands are supported");

    let output = std::process::Command::new(graft_binary())
        .args(&parts[1..])
        .current_dir(dir)
        .output()
        .expect("failed to execute graft binary");

    world.exit_code = output.status.code();
    world.stdout = String::from_utf8_lossy(&output.stdout).to_string();
    world.stderr = String::from_utf8_lossy(&output.stderr).to_string();
}

// --- Then steps ---

#[then("the command should succeed")]
fn command_should_succeed(world: &mut GraftWorld) {
    let code = world.exit_code.expect("no exit code captured");
    assert_eq!(
        code, 0,
        "expected success (exit 0) but got {code}\nstdout: {}\nstderr: {}",
        world.stdout, world.stderr
    );
}

#[then("the command should fail")]
fn command_should_fail(world: &mut GraftWorld) {
    let code = world.exit_code.expect("no exit code captured");
    assert_ne!(
        code, 0,
        "expected failure (non-zero exit) but got 0\nstdout: {}\nstderr: {}",
        world.stdout, world.stderr
    );
}

#[then(expr = "a file {string} should exist")]
fn file_should_exist(world: &mut GraftWorld, filename: String) {
    let dir = world.work_dir.as_ref().expect("work_dir not set");
    let path = dir.join(&filename);
    assert!(
        path.exists(),
        "expected file {filename} to exist at {path:?}"
    );
}

#[then(expr = "{string} should contain {string}")]
fn file_should_contain(world: &mut GraftWorld, filename: String, expected: String) {
    let dir = world.work_dir.as_ref().expect("work_dir not set");
    let path = dir.join(&filename);
    let content =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {path:?}: {e}"));
    assert!(
        content.contains(&expected),
        "expected {filename} to contain {expected:?}, got:\n{content}"
    );
}

#[then(expr = "stdout should contain {string}")]
fn stdout_should_contain(world: &mut GraftWorld, expected: String) {
    assert!(
        world.stdout.contains(&expected),
        "expected stdout to contain {expected:?}, got:\n{}",
        world.stdout
    );
}

#[then(expr = "stderr should contain {string}")]
fn stderr_should_contain(world: &mut GraftWorld, expected: String) {
    assert!(
        world.stderr.contains(&expected),
        "expected stderr to contain {expected:?}, got:\n{}",
        world.stderr
    );
}

fn main() {
    futures::executor::block_on(GraftWorld::run("tests/features"));
}
