//! Extraction of user-selected controller-pack archives.
//!
//! AeroNav (GNG) packages are distributed as `.zip` / `.7z` files. Rather than
//! downloading them automatically, the user picks the archives for the FIRs
//! they want and we extract each one into a tempdir. The extracted roots are
//! then fed to `pack_sync::plan` exactly like before — only the *source* of the
//! archives changed.

use anyhow::Context;
use std::fs::File;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// One extracted package, kept alive (via its tempdir) for the lifetime of the
/// sync run.
pub struct LocalPackage {
    _tmp: TempDir,
    root: PathBuf,
}

impl LocalPackage {
    pub fn root(&self) -> &Path {
        &self.root
    }
}

/// Extract a `.zip` or `.7z` archive into a fresh tempdir.
pub fn extract_package(path: &Path) -> anyhow::Result<LocalPackage> {
    let tmp = TempDir::new().context("create tempdir for package")?;
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    match ext.as_str() {
        "zip" => {
            let file =
                File::open(path).with_context(|| format!("open {}", path.display()))?;
            let mut archive = zip::ZipArchive::new(file)?;
            archive.extract(tmp.path())?;
        }
        "7z" => {
            sevenz_rust::decompress_file(path, tmp.path())
                .with_context(|| format!("extract {}", path.display()))?;
        }
        other => anyhow::bail!("unsupported package type '.{other}' (expected .zip or .7z)"),
    }

    let root = tmp.path().to_path_buf();
    Ok(LocalPackage { _tmp: tmp, root })
}
