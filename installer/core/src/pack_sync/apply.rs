use super::plan::{summarize, FileOp, SyncPlan, SyncSummary};
use std::fs;
use std::io;
use std::path::Path;

pub fn apply(install_root: &Path, plan: &SyncPlan) -> anyhow::Result<SyncSummary> {
    let mut written = 0usize;
    let mut skipped = 0usize;

    for op in &plan.ops {
        match apply_one(install_root, op) {
            Ok(true) => written += 1,
            Ok(false) => skipped += 1,
            Err(e) => {
                tracing::error!(?op, error = %e, "operation failed; aborting");
                return Err(e);
            }
        }
    }

    Ok(summarize(plan, written, skipped))
}

fn apply_one(install_root: &Path, op: &FileOp) -> anyhow::Result<bool> {
    match op {
        FileOp::EnsureDir { path } => {
            fs::create_dir_all(path)?;
            Ok(false)
        }
        FileOp::Copy { src, dst } => {
            if same_content(src, dst)? {
                return Ok(false);
            }
            ensure_parent(dst)?;
            atomic_copy(src, dst)?;
            Ok(true)
        }
        FileOp::WriteSector { src, dst, .. } => {
            if same_content(src, dst)? {
                return Ok(false);
            }
            ensure_parent(dst)?;
            if dst.exists() {
                fs::remove_file(dst)?;
            }
            atomic_copy(src, dst)?;
            Ok(true)
        }
        FileOp::BackupSector { src, dst } => {
            if !src.exists() {
                return Ok(false);
            }
            ensure_parent(dst)?;
            match fs::rename(src, dst) {
                Ok(()) => Ok(true),
                Err(_) => {
                    fs::copy(src, dst)?;
                    fs::remove_file(src)?;
                    Ok(true)
                }
            }
        }
        FileOp::MoveProfile { src, dst } => {
            if src == dst {
                return Ok(false);
            }
            if same_content(src, dst)? {
                return Ok(false);
            }
            ensure_parent(dst)?;
            atomic_copy(src, dst)?;
            Ok(true)
        }
        FileOp::WriteText { dst, value } => {
            ensure_parent(dst)?;
            if dst.exists() {
                if let Ok(existing) = fs::read_to_string(dst) {
                    if existing.trim() == value.trim() {
                        return Ok(false);
                    }
                }
            }
            atomic_write(dst, value.as_bytes())?;
            Ok(true)
        }
        FileOp::DeleteLegacy { path } => {
            if !path.exists() {
                return Ok(false);
            }
            if path.is_dir() {
                fs::remove_dir_all(path)?;
            } else {
                fs::remove_file(path)?;
            }
            Ok(true)
        }
    }
    .map_err(|e: anyhow::Error| {
        tracing::warn!(?install_root, "apply step failed");
        e
    })
}

fn ensure_parent(p: &Path) -> io::Result<()> {
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn atomic_copy(src: &Path, dst: &Path) -> io::Result<()> {
    let tmp = dst.with_extension(format!(
        "{}.partial",
        dst.extension()
            .map(|e| e.to_string_lossy().into_owned())
            .unwrap_or_default()
    ));
    fs::copy(src, &tmp)?;
    if let Err(e) = fs::rename(&tmp, dst) {
        // On Windows, rename across the same dir works; if rename fails (e.g.
        // dst exists with file lock), fall back to direct copy.
        fs::copy(src, dst)?;
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(())
}

fn atomic_write(dst: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = dst.with_extension("partial.tmp");
    fs::write(&tmp, bytes)?;
    if let Err(e) = fs::rename(&tmp, dst) {
        fs::write(dst, bytes)?;
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(())
}

fn same_content(src: &Path, dst: &Path) -> io::Result<bool> {
    let dst_meta = match fs::metadata(dst) {
        Ok(m) => m,
        Err(_) => return Ok(false),
    };
    let src_meta = fs::metadata(src)?;
    if src_meta.len() != dst_meta.len() {
        return Ok(false);
    }
    let src_bytes = fs::read(src)?;
    let dst_bytes = fs::read(dst)?;
    Ok(src_bytes == dst_bytes)
}
