use std::collections::HashMap;
use std::io::Read;
use std::process::Command;

use base64::prelude::*;
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, USER_AGENT};
use serde_json::Value;

use crate::error::{GraftError, Result};

pub struct GitHubClient {
    client: Client,
}

impl GitHubClient {
    /// Create a new GitHub API client.
    ///
    /// If `token` is provided, all requests will include an `Authorization: Bearer` header.
    /// The `User-Agent` header is always set to `graft/{version}`.
    pub fn new(token: Option<String>) -> Self {
        let version = env!("CARGO_PKG_VERSION");
        let ua = format!("graft/{version}");

        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_str(&ua).expect("valid user-agent"),
        );
        if let Some(ref tok) = token {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {tok}")).expect("valid auth header"),
            );
        }

        let client = Client::builder()
            .default_headers(headers)
            .build()
            .expect("failed to build reqwest client");

        Self { client }
    }

    /// Resolve a tag or SHA to a full commit SHA.
    pub fn resolve_ref(&self, owner: &str, repo: &str, git_ref: &str) -> Result<String> {
        let repo_slug = format!("{owner}/{repo}");

        // Heuristic: if it looks like a hex SHA (>= 7 chars, all hex), resolve as commit.
        if git_ref.len() >= 7 && git_ref.chars().all(|c| c.is_ascii_hexdigit()) {
            let url = format!("https://api.github.com/repos/{owner}/{repo}/commits/{git_ref}");
            let resp = self
                .client
                .get(&url)
                .send()
                .map_err(|e| GraftError::Network {
                    url: url.clone(),
                    source: e,
                })?;

            let status = resp.status().as_u16();
            match status {
                200 => {
                    let body: Value = resp
                        .json()
                        .map_err(|e| GraftError::Network { url, source: e })?;
                    body["sha"]
                        .as_str()
                        .map(|s: &str| s.to_string())
                        .ok_or_else(|| GraftError::GitHubApi {
                            status,
                            message: "missing sha in commit response".to_string(),
                        })
                }
                401 | 403 => Err(GraftError::AuthFailed { repo: repo_slug }),
                404 => Err(GraftError::SourcePathNotFound {
                    path: git_ref.to_string(),
                    repo: repo_slug,
                    version: git_ref.to_string(),
                }),
                _ => {
                    let text = resp.text().unwrap_or_default();
                    Err(GraftError::GitHubApi {
                        status,
                        message: text,
                    })
                }
            }
        } else {
            // Treat as a tag.
            let url = format!("https://api.github.com/repos/{owner}/{repo}/git/ref/tags/{git_ref}");
            let resp = self
                .client
                .get(&url)
                .send()
                .map_err(|e| GraftError::Network {
                    url: url.clone(),
                    source: e,
                })?;

            let status = resp.status().as_u16();
            match status {
                200 => {}
                401 | 403 => return Err(GraftError::AuthFailed { repo: repo_slug }),
                404 => {
                    return Err(GraftError::TagNotFound {
                        tag: git_ref.to_string(),
                        repo: repo_slug,
                    })
                }
                _ => {
                    let text = resp.text().unwrap_or_default();
                    return Err(GraftError::GitHubApi {
                        status,
                        message: text,
                    });
                }
            }

            let body: Value = resp
                .json()
                .map_err(|e| GraftError::Network { url, source: e })?;

            let obj_type = body["object"]["type"].as_str().unwrap_or_default();
            let obj_sha = body["object"]["sha"]
                .as_str()
                .unwrap_or_default()
                .to_string();

            if obj_type == "commit" {
                Ok(obj_sha)
            } else if obj_type == "tag" {
                // Annotated tag — dereference.
                let tag_url =
                    format!("https://api.github.com/repos/{owner}/{repo}/git/tags/{obj_sha}");
                let tag_resp =
                    self.client
                        .get(&tag_url)
                        .send()
                        .map_err(|e| GraftError::Network {
                            url: tag_url.clone(),
                            source: e,
                        })?;

                let tag_status = tag_resp.status().as_u16();
                if !tag_resp.status().is_success() {
                    let text = tag_resp.text().unwrap_or_default();
                    return Err(GraftError::GitHubApi {
                        status: tag_status,
                        message: text,
                    });
                }

                let tag_body: Value = tag_resp.json().map_err(|e| GraftError::Network {
                    url: tag_url,
                    source: e,
                })?;

                tag_body["object"]["sha"]
                    .as_str()
                    .map(|s: &str| s.to_string())
                    .ok_or_else(|| GraftError::GitHubApi {
                        status: 200,
                        message: "missing sha in annotated tag response".to_string(),
                    })
            } else {
                Err(GraftError::GitHubApi {
                    status: 200,
                    message: format!("unexpected ref object type: {obj_type}"),
                })
            }
        }
    }

    /// Fetch a single file's contents via the GitHub Contents API.
    pub fn fetch_file(
        &self,
        owner: &str,
        repo: &str,
        path: &str,
        git_ref: &str,
    ) -> Result<Vec<u8>> {
        let url =
            format!("https://api.github.com/repos/{owner}/{repo}/contents/{path}?ref={git_ref}");
        let resp = self
            .client
            .get(&url)
            .send()
            .map_err(|e| GraftError::Network {
                url: url.clone(),
                source: e,
            })?;

        let status = resp.status().as_u16();
        match status {
            200 => {}
            401 | 403 => {
                return Err(GraftError::AuthFailed {
                    repo: format!("{owner}/{repo}"),
                })
            }
            404 => {
                return Err(GraftError::SourcePathNotFound {
                    path: path.to_string(),
                    repo: format!("{owner}/{repo}"),
                    version: git_ref.to_string(),
                })
            }
            _ => {
                let text = resp.text().unwrap_or_default();
                return Err(GraftError::GitHubApi {
                    status,
                    message: text,
                });
            }
        }

        let body: Value = resp.json().map_err(|e| GraftError::Network {
            url: url.clone(),
            source: e,
        })?;

        let content_b64 = body["content"]
            .as_str()
            .ok_or_else(|| GraftError::GitHubApi {
                status: 200,
                message: "missing content field".to_string(),
            })?;

        // GitHub base64 content contains newlines — strip them before decoding.
        let cleaned: String = content_b64
            .chars()
            .filter(|c: &char| !c.is_whitespace())
            .collect();

        BASE64_STANDARD
            .decode(&cleaned)
            .map_err(|e| GraftError::GitHubApi {
                status: 200,
                message: format!("base64 decode error: {e}"),
            })
    }

    /// Fetch a directory's contents by downloading the repo tarball and extracting matching entries.
    pub fn fetch_directory(
        &self,
        owner: &str,
        repo: &str,
        path: &str,
        git_ref: &str,
        files_filter: Option<&[String]>,
    ) -> Result<Vec<(String, Vec<u8>)>> {
        let url = format!("https://api.github.com/repos/{owner}/{repo}/tarball/{git_ref}");
        let resp = self
            .client
            .get(&url)
            .send()
            .map_err(|e| GraftError::Network {
                url: url.clone(),
                source: e,
            })?;

        let status = resp.status().as_u16();
        match status {
            200 => {}
            401 | 403 => {
                return Err(GraftError::AuthFailed {
                    repo: format!("{owner}/{repo}"),
                })
            }
            404 => {
                return Err(GraftError::SourcePathNotFound {
                    path: path.to_string(),
                    repo: format!("{owner}/{repo}"),
                    version: git_ref.to_string(),
                })
            }
            _ => {
                let text = resp.text().unwrap_or_default();
                return Err(GraftError::GitHubApi {
                    status,
                    message: text,
                });
            }
        }

        let bytes = resp.bytes().map_err(|e| GraftError::Network {
            url: url.clone(),
            source: e,
        })?;

        let decoder = flate2::read::GzDecoder::new(&bytes[..]);
        let mut archive = tar::Archive::new(decoder);

        // Normalise the target path: ensure no trailing slash for prefix matching.
        let prefix = path.trim_end_matches('/');

        let mut results = Vec::new();

        for entry in archive.entries().map_err(|e| GraftError::Io {
            context: "reading tarball entries".to_string(),
            source: e,
        })? {
            let mut entry = entry.map_err(|e| GraftError::Io {
                context: "reading tarball entry".to_string(),
                source: e,
            })?;

            let entry_path = entry
                .path()
                .map_err(|e| GraftError::Io {
                    context: "reading entry path".to_string(),
                    source: e,
                })?
                .to_string_lossy()
                .to_string();

            // Strip the first component (e.g. "owner-repo-sha/").
            let stripped = match entry_path.find('/') {
                Some(idx) => &entry_path[idx + 1..],
                None => continue,
            };

            // Must be under the target directory.
            if !stripped.starts_with(prefix) {
                continue;
            }

            // Skip directories themselves.
            if entry.header().entry_type().is_dir() {
                continue;
            }

            // Compute relative path within the target directory.
            let relative = stripped
                .strip_prefix(prefix)
                .unwrap_or(stripped)
                .trim_start_matches('/');

            if relative.is_empty() {
                continue;
            }

            // Apply files filter if provided.
            if let Some(filter) = files_filter {
                let filename = relative.rsplit('/').next().unwrap_or(relative);
                if !filter.iter().any(|f| f == filename) {
                    continue;
                }
            }

            let mut content = Vec::new();
            entry
                .read_to_end(&mut content)
                .map_err(|e| GraftError::Io {
                    context: format!("reading tarball entry {stripped}"),
                    source: e,
                })?;

            results.push((relative.to_string(), content));
        }

        Ok(results)
    }

    /// List remote tags using `git ls-remote`.
    ///
    /// Returns `(tag_name, commit_sha)` pairs. For annotated tags, the dereferenced
    /// commit SHA (from the `^{}` entry) is used.
    pub fn ls_remote_tags(&self, owner: &str, repo: &str) -> Result<Vec<(String, String)>> {
        let url = format!("https://github.com/{owner}/{repo}");

        let output = Command::new("git")
            .args(["ls-remote", "--tags", &url])
            .output()
            .map_err(|_| GraftError::GitNotFound)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // If it looks like an auth failure, report that.
            if stderr.contains("Authentication") || stderr.contains("403") || stderr.contains("401")
            {
                return Err(GraftError::AuthFailed {
                    repo: format!("{owner}/{repo}"),
                });
            }
            return Err(GraftError::GitHubApi {
                status: 0,
                message: format!("git ls-remote failed: {stderr}"),
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        // First pass: collect all entries, preferring ^{} dereferenced SHAs.
        let mut tag_map: HashMap<String, String> = HashMap::new();
        let mut seen_deref: std::collections::HashSet<String> = std::collections::HashSet::new();

        for line in stdout.lines() {
            let mut parts = line.splitn(2, '\t');
            let sha = match parts.next() {
                Some(s) => s.trim().to_string(),
                None => continue,
            };
            let refname = match parts.next() {
                Some(s) => s.trim(),
                None => continue,
            };

            let tag_name = match refname.strip_prefix("refs/tags/") {
                Some(name) => name.to_string(),
                None => continue,
            };

            if let Some(base) = tag_name.strip_suffix("^{}") {
                // Dereferenced entry — this is the commit SHA we want.
                tag_map.insert(base.to_string(), sha);
                seen_deref.insert(base.to_string());
            } else if !seen_deref.contains(&tag_name) {
                tag_map.insert(tag_name, sha);
            }
        }

        let mut tags: Vec<(String, String)> = tag_map.into_iter().collect();
        tags.sort_by(|a, b| a.0.cmp(&b.0));

        Ok(tags)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let _client = GitHubClient::new(None);
    }

    #[test]
    fn test_client_creation_with_token() {
        let _client = GitHubClient::new(Some("test-token".to_string()));
    }
}
