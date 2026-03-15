use cucumber::World;
use std::path::PathBuf;

#[derive(Debug, Default, World)]
pub struct GraftWorld {
    /// Temp directory for test isolation
    pub work_dir: Option<PathBuf>,
    /// Last command's exit code
    pub exit_code: Option<i32>,
    /// Last command's stdout
    pub stdout: String,
    /// Last command's stderr
    pub stderr: String,
}

fn main() {
    futures::executor::block_on(GraftWorld::run("tests/features"));
}
