use std::path::Path;

use console::style;

pub fn run() -> miette::Result<()> {
    let path = Path::new("graft.toml");
    if path.exists() {
        println!(
            "{} graft.toml already exists",
            style("skip").yellow().bold()
        );
        return Ok(());
    }

    let content = r#"# Graft — package manager for config files
# Docs: https://github.com/rroskam/graft
#
# [deps.example]
# source = "gh:owner/repo/path/to/file"
# version = "v1.0.0"
# dest = "local/path/to/file"
"#;

    std::fs::write(path, content).map_err(|e| graft::error::GraftError::Io {
        context: "writing graft.toml".into(),
        source: e,
    })?;

    println!("{} Created graft.toml", style("done").green().bold());
    Ok(())
}
