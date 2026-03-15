use std::process::Command;

/// Resolve a GitHub token from the environment or the `gh` CLI.
///
/// Tries, in order:
/// 1. `GH_TOKEN` env var
/// 2. `GITHUB_TOKEN` env var
/// 3. `gh auth token` command output
/// 4. `None` (unauthenticated)
pub fn resolve_token() -> Option<String> {
    // 1. GH_TOKEN
    if let Ok(val) = std::env::var("GH_TOKEN") {
        if !val.is_empty() {
            return Some(val);
        }
    }

    // 2. GITHUB_TOKEN
    if let Ok(val) = std::env::var("GITHUB_TOKEN") {
        if !val.is_empty() {
            return Some(val);
        }
    }

    // 3. gh auth token
    if let Ok(output) = Command::new("gh").args(["auth", "token"]).output() {
        if output.status.success() {
            let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !token.is_empty() {
                return Some(token);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_token_does_not_panic() {
        // Just ensure it doesn't panic — the actual value depends on the environment.
        let _ = resolve_token();
    }
}
