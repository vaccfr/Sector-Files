use anyhow::Context;
use serde::Deserialize;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

pub const OWNER: &str = "vaccfr";
pub const REPO: &str = "Sector-Files";
pub const BRANCH: &str = "main";

/// Repo paths we never want in a controller-pack install: dev tooling and repo
/// metadata. Matched against the repo-relative path (forward slashes), so the
/// folders match by their first segment and the loose files by exact name.
fn is_excluded_repo_path(repo_rel: &str) -> bool {
    let first = repo_rel.split('/').next().unwrap_or("");
    matches!(first, "scripts" | "installer" | ".github")
        || repo_rel.eq_ignore_ascii_case(".gitignore")
        || repo_rel.eq_ignore_ascii_case("README.md")
}

#[derive(Debug, Deserialize)]
struct CommitResponse {
    sha: String,
}

fn user_agent() -> String {
    format!(
        "vaccfr-controller-pack-installer/{}",
        env!("CARGO_PKG_VERSION")
    )
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(user_agent())
        .build()
        .expect("reqwest client")
}

pub async fn get_short_sha() -> anyhow::Result<String> {
    let url = format!("https://api.github.com/repos/{OWNER}/{REPO}/commits/{BRANCH}");
    let resp = client().get(&url).send().await?.error_for_status()?;
    let body: CommitResponse = resp.json().await?;
    Ok(body.sha.chars().take(7).collect())
}

/// Download the GitHub repo zipball into a tempdir and return the path to the
/// extracted top-level directory.
///
/// GitHub's zipball API has no partial-download option, so we still fetch one
/// archive — but we skip extracting dev/meta paths (see `is_excluded_repo_path`)
/// so they never hit disk or the install overlay.
pub async fn download_repo() -> anyhow::Result<DownloadedRepo> {
    let url = format!("https://api.github.com/repos/{OWNER}/{REPO}/zipball/{BRANCH}");
    let bytes = client()
        .get(&url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;

    let tmp = TempDir::new().context("create tempdir for github zipball")?;
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))?;

    // GitHub zipballs wrap everything in a single top-level directory like
    // `vaccfr-Sector-Files-<sha>/`. We preserve that wrapper and extract only
    // the entries we care about beneath it.
    let mut root: Option<PathBuf> = None;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = entry.name().to_string();
        let (wrapper, repo_rel) = match name.split_once('/') {
            Some((w, rest)) => (w, rest),
            // The wrapper directory entry itself ("wrapper/") or a stray
            // top-level entry — nothing to extract, but note the wrapper.
            None => (name.trim_end_matches('/'), ""),
        };
        root.get_or_insert_with(|| tmp.path().join(wrapper));

        if repo_rel.is_empty() || is_excluded_repo_path(repo_rel) {
            continue;
        }

        let out = tmp.path().join(wrapper).join(repo_rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out)?;
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = std::fs::File::create(&out)?;
        std::io::copy(&mut entry, &mut file)?;
    }

    let root = root.context("github zipball did not contain a top-level directory")?;
    Ok(DownloadedRepo { _tmp: tmp, root })
}

pub struct DownloadedRepo {
    _tmp: TempDir,
    root: PathBuf,
}

impl DownloadedRepo {
    pub fn root(&self) -> &Path {
        &self.root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excludes_dev_and_meta_paths() {
        assert!(is_excluded_repo_path("scripts/install.py"));
        assert!(is_excluded_repo_path("installer/src-tauri/main.rs"));
        assert!(is_excluded_repo_path(".github/workflows/ci.yml"));
        assert!(is_excluded_repo_path(".gitignore"));
        assert!(is_excluded_repo_path("README.md"));
        assert!(is_excluded_repo_path("readme.md"));
    }

    #[test]
    fn keeps_pack_content() {
        assert!(!is_excluded_repo_path("LFBB/ASR/Tower.asr"));
        assert!(!is_excluded_repo_path("LFXX/Settings/Symbology.txt"));
        assert!(!is_excluded_repo_path("LFBB/README.md")); // only top-level README is dropped
    }
}
