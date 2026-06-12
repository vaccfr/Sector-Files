use super::airac::{parse_gng_sector_target, LFXXN_CODE};
use super::ownership::{gng_owned_set, is_gng_owned};
use super::{COPYRIGHT_FILE, CURRENT_AIRAC_FILE, SECTORS_SUBPATH, SECTOR_BACKUP_DIRNAME};
use crate::fir::FirCode;
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileOp {
    /// Copy a file from one location to another. Used for the GitHub overlay
    /// and for non-sector GNG files (e.g. ICAO, NavData).
    Copy { src: PathBuf, dst: PathBuf },
    /// Move an existing installed sector file to the backup directory before
    /// writing a new one in its place.
    BackupSector { src: PathBuf, dst: PathBuf },
    /// Write a sector file to its canonical location, renamed to <FIR>.<ext>.
    WriteSector { src: PathBuf, dst: PathBuf, fir: FirCode, ext: SectorExt },
    /// Move a `.prf` file to the FIR folder root.
    MoveProfile { src: PathBuf, dst: PathBuf },
    /// Write/overwrite a marker text file with a given string value.
    WriteText { dst: PathBuf, value: String },
    /// Ensure a directory exists.
    EnsureDir { path: PathBuf },
    /// Delete a legacy file (e.g. root-level `.sct` from old layouts).
    DeleteLegacy { path: PathBuf },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum SectorExt {
    Sct,
    Ese,
}

impl SectorExt {
    pub fn as_str(self) -> &'static str {
        match self {
            SectorExt::Sct => "sct",
            SectorExt::Ese => "ese",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "sct" => Some(SectorExt::Sct),
            "ese" => Some(SectorExt::Ese),
            _ => None,
        }
    }
}

#[derive(Debug, Default)]
pub struct SyncPlan {
    pub ops: Vec<FileOp>,
    pub detected_airac: Option<String>,
    pub previous_airac: Option<String>,
    pub github_short_sha: Option<String>,
    /// Real, user-facing warnings (surfaced in the sync summary).
    pub warnings: Vec<String>,
    /// Diagnostic notes for things we skipped on purpose (e.g. non-FIR sector
    /// files). Logged for debugging but NOT shown to the user as warnings.
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct SyncSummary {
    pub github_sha: Option<String>,
    pub airac_cycle: Option<String>,
    pub files_written: usize,
    pub files_skipped: usize,
    pub warnings: Vec<String>,
}

/// The legacy/Tier-2 "secret" area folder. It lives on GitHub but is only
/// installed when a matching package is selected (see `detect_installed_codes`).
pub const LFFM_CODE: &str = "LFFM";

/// How the set of areas (FIR folders + LFFM) to install is determined.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AreaSource {
    /// Areas come from the selected packages. The packages scope which FIR
    /// folders get (re)written; folders already on disk for areas *not* in the
    /// selection are left untouched (never removed). Used for a full
    /// install/update.
    #[default]
    Packages,
    /// Areas are whatever is already present in the install root, and nothing is
    /// removed. Used to refresh GitHub files in place without re-supplying the
    /// FIR packages.
    InstalledOnly,
}

#[derive(Debug)]
pub struct PlanInputs<'a> {
    pub github_root: Option<&'a Path>,
    pub gng_roots: &'a [PathBuf],
    pub install_root: &'a Path,
    pub github_short_sha: Option<String>,
    pub area_source: AreaSource,
}

/// Uppercased first path segment, e.g. `LFBB/ICAO/x` → `Some("LFBB")`.
fn first_segment_upper(rel: &Path) -> Option<String> {
    rel.components()
        .next()
        .map(|c| c.as_os_str().to_string_lossy().to_ascii_uppercase())
}

/// Which areas the selected packages provide. The GitHub overlay keeps only
/// these folders (plus the always-shared `LFXX`); everything else is dropped.
///
/// Detection is by content: a `<CODE>/` folder, or a `<CODE>-…` / `<CODE>.…`
/// sector/profile filename. `LFFM` is tracked separately since it is not a
/// regular [`FirCode`] and stays excluded unless its package is present.
fn detect_installed_codes(gng_roots: &[PathBuf]) -> (BTreeSet<FirCode>, bool) {
    let mut firs = BTreeSet::new();
    let mut lffm = false;
    let matches_code = |upper: &str, code: &str| {
        upper == code || upper.starts_with(&format!("{code}-")) || upper.starts_with(&format!("{code}."))
    };
    for root in gng_roots {
        for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
            let upper = entry.file_name().to_string_lossy().to_ascii_uppercase();
            for fir in FirCode::ALL {
                if matches_code(&upper, fir.as_str()) {
                    firs.insert(fir);
                }
            }
            // The combined LFXXN package covers both LFFF and LFEE. LFXXN is not
            // a FIR (and no FIR code is a prefix of it), so it is matched
            // explicitly and expands to both covered FIRs.
            if matches_code(&upper, LFXXN_CODE) {
                firs.insert(FirCode::LFFF);
                firs.insert(FirCode::LFEE);
            }
            if matches_code(&upper, LFFM_CODE) {
                lffm = true;
            }
        }
    }
    (firs, lffm)
}

/// Which areas already have a top-level folder in the install root. Used by a
/// GitHub-only refresh to know which folders to update without packages.
fn detect_installed_areas_on_disk(install_root: &Path) -> (BTreeSet<FirCode>, bool) {
    let mut firs = BTreeSet::new();
    let mut lffm = false;
    if let Ok(entries) = std::fs::read_dir(install_root) {
        for entry in entries.flatten() {
            if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let upper = entry.file_name().to_string_lossy().to_ascii_uppercase();
            if let Ok(fir) = upper.parse::<FirCode>() {
                firs.insert(fir);
            }
            if upper == LFFM_CODE {
                lffm = true;
            }
        }
    }
    (firs, lffm)
}

/// If `rel` is `<FIR>/ICAO/…` or `<FIR>/NavData/…`, the equivalent path under
/// `LFXX` (CoFrance reads ICAO/NavData from `LFXX`, not the per-FIR folder).
fn mirror_navdata_to_lfxx(rel: &Path) -> Option<PathBuf> {
    let parts: Vec<_> = rel.iter().collect();
    if parts.len() < 2 {
        return None;
    }
    let first = parts[0].to_string_lossy();
    let second = parts[1].to_string_lossy();
    let is_fir = first.parse::<FirCode>().is_ok();
    let is_navdata = second.eq_ignore_ascii_case("ICAO") || second.eq_ignore_ascii_case("NavData");
    if is_fir && is_navdata {
        let tail: PathBuf = parts[1..].iter().collect();
        Some(Path::new("LFXX").join(tail))
    } else {
        None
    }
}

pub fn plan(inputs: PlanInputs<'_>) -> anyhow::Result<SyncPlan> {
    let mut plan = SyncPlan {
        github_short_sha: inputs.github_short_sha.clone(),
        previous_airac: read_current_airac(inputs.install_root),
        ..Default::default()
    };
    let gng_set = gng_owned_set();

    // Which areas to install. From the packages (full sync) or from whatever is
    // already on disk (GitHub-only refresh). `LFXX` is always kept; `LFFM` only
    // when present in the chosen source.
    let (installed_firs, install_lffm) = match inputs.area_source {
        AreaSource::Packages => detect_installed_codes(inputs.gng_roots),
        AreaSource::InstalledOnly => detect_installed_areas_on_disk(inputs.install_root),
    };

    // 0) A full install never deletes top-level FIR folders. The selected
    //    packages only *scope* which folders get (re)written below — they do not
    //    prune anything. A folder already on disk for an area not in this run's
    //    selection was put there by a previous install; the user simply not
    //    re-picking its package this time must not destroy their working setup
    //    (their ASRs, custom files, credentials, etc.). Deleting was only ever
    //    safe on a first-time install, where nothing exists to lose — so we drop
    //    it entirely and leave uncovered areas untouched. (`installed_firs` /
    //    `install_lffm` still gate the GitHub overlay below.) A GitHub-only
    //    refresh likewise removes nothing.

    // 1) Back up the current LFXX/Settings, then lay down the GitHub overlay
    //    (which re-downloads and would otherwise overwrite those settings).
    if let Some(github_root) = inputs.github_root {
        plan_settings_backup(inputs.install_root, &mut plan);
        plan_github_overlay(
            github_root,
            inputs.install_root,
            &gng_set,
            &installed_firs,
            install_lffm,
            &mut plan,
        );
    }

    // 2) Plan GNG-source operations across all extracted packages. Sector
    //    writes are collected separately so they can be ordered AFTER their
    //    matching BackupSector ops in step 3.
    let mut sector_writes: Vec<FileOp> = Vec::new();
    let mut sector_targets: BTreeSet<(FirCode, SectorExt)> = BTreeSet::new();
    for gng_root in inputs.gng_roots {
        plan_gng_overlay(
            gng_root,
            inputs.install_root,
            &mut plan,
            &mut sector_writes,
            &mut sector_targets,
        );
    }

    // Filter no-op sector writes (src content matches the file already at dst).
    sector_writes.retain(|op| match op {
        FileOp::WriteSector { src, dst, fir, ext } => {
            if files_equal(src, dst) {
                sector_targets.remove(&(*fir, *ext));
                false
            } else {
                true
            }
        }
        _ => true,
    });

    // 3) Backup existing sector files for sectors that will actually be overwritten.
    if !sector_targets.is_empty() {
        let previous_airac = plan.previous_airac.clone();
        plan_sector_backups(
            inputs.install_root,
            &sector_targets,
            previous_airac.as_deref(),
            &mut plan,
        );
    }

    // 4) Now append the sector writes AFTER backups.
    plan.ops.extend(sector_writes);

    // 5) AIRAC marker — only write if we have a parsed cycle.
    if let Some(cycle) = &plan.detected_airac {
        plan.ops.push(FileOp::WriteText {
            dst: inputs.install_root.join(CURRENT_AIRAC_FILE),
            value: cycle.clone(),
        });
    } else if !sector_targets.is_empty() {
        plan.warnings.push(
            "Sector files present but no AIRAC cycle could be parsed; current_airac.txt left unchanged"
                .into(),
        );
    }

    // 6) GitHub installer-version marker, if we synced from GitHub.
    if let Some(sha) = &inputs.github_short_sha {
        plan.ops.push(FileOp::WriteText {
            dst: inputs.install_root.join(super::INSTALLER_VERSION_FILE),
            value: sha.clone(),
        });
    }

    Ok(plan)
}

fn files_equal(a: &Path, b: &Path) -> bool {
    let (am, bm) = match (std::fs::metadata(a), std::fs::metadata(b)) {
        (Ok(am), Ok(bm)) => (am, bm),
        _ => return false,
    };
    if am.len() != bm.len() {
        return false;
    }
    match (std::fs::read(a), std::fs::read(b)) {
        (Ok(ab), Ok(bb)) => ab == bb,
        _ => false,
    }
}

/// Snapshot the current `LFXX/Settings` into `LFXX/Settings/backup` before the
/// GitHub overlay overwrites it. Emitted before the overlay ops so the copies
/// capture the *old* files; the `backup/` folder itself is never re-backed-up.
fn plan_settings_backup(install_root: &Path, plan: &mut SyncPlan) {
    let settings_dir = install_root.join("LFXX").join("Settings");
    if !settings_dir.is_dir() {
        return;
    }
    let backup_dir = settings_dir.join("backup");
    for entry in WalkDir::new(&settings_dir).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.starts_with(&backup_dir) {
            continue;
        }
        let Ok(rel) = path.strip_prefix(&settings_dir) else {
            continue;
        };
        plan.ops.push(FileOp::Copy {
            src: path.to_path_buf(),
            dst: backup_dir.join(rel),
        });
    }
}

fn plan_github_overlay(
    github_root: &Path,
    install_root: &Path,
    gng_set: &globset::GlobSet,
    installed_firs: &BTreeSet<FirCode>,
    install_lffm: bool,
    plan: &mut SyncPlan,
) {
    for entry in WalkDir::new(github_root).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = match entry.path().strip_prefix(github_root) {
            Ok(p) => p.to_path_buf(),
            Err(_) => continue,
        };

        // Keep only area folders the packages cover. `LFXX` and any non-area
        // top-level files are always kept; `LFFM` only when its package is in.
        if let Some(code) = first_segment_upper(&rel) {
            let skip = if code == LFFM_CODE {
                !install_lffm
            } else if let Ok(fir) = code.parse::<FirCode>() {
                !installed_firs.contains(&fir)
            } else {
                false
            };
            if skip {
                continue;
            }
        }

        // Skip duplicate copyright files at non-FIR locations; handled
        // separately for the per-FIR rule below.
        let file_name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if file_name == COPYRIGHT_FILE {
            // Always keep per-FIR copyright if present; the overlay handles it.
        }

        // Package-owned paths (sectors, ICAO, NavData) are provided by the
        // packages, not GitHub.
        if is_gng_owned(gng_set, &rel) {
            continue;
        }

        plan.ops.push(FileOp::Copy {
            src: entry.path().to_path_buf(),
            dst: install_root.join(&rel),
        });
    }
}

fn plan_gng_overlay(
    gng_root: &Path,
    install_root: &Path,
    plan: &mut SyncPlan,
    sector_writes: &mut Vec<FileOp>,
    sector_targets: &mut BTreeSet<(FirCode, SectorExt)>,
) {
    let sectors_dir = install_root.join(SECTORS_SUBPATH);

    for entry in WalkDir::new(gng_root).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase());

        // Sector files: rename to <FIR>.<ext>, place in LFXX/Sectors/. A combined
        // code (e.g. LFXXN) fans the single source file out to one renamed sector
        // per covered FIR.
        if let Some(ext) = ext.as_deref().and_then(SectorExt::from_str) {
            if let Some((firs, cycle)) = parse_gng_sector_target(&name) {
                if let Some(c) = cycle.clone() {
                    plan.detected_airac = Some(c);
                }
                for fir in firs {
                    let dst = sectors_dir.join(format!("{}.{}", fir.as_str(), ext.as_str()));
                    sector_writes.push(FileOp::WriteSector {
                        src: path.to_path_buf(),
                        dst,
                        fir,
                        ext,
                    });
                    sector_targets.insert((fir, ext));
                }
                continue;
            }
            // Not a FIR sector filename — skip (e.g. sub-sector includes). This
            // is expected, so it's a diagnostic note rather than a warning.
            plan.notes
                .push(format!("Ignored non-FIR sector file: {}", name));
            continue;
        }

        // `.rwy` files: keep, paired with the sector dir as <FIR>.rwy. A combined
        // code fans the single source out to one <FIR>.rwy per covered FIR.
        if ext.as_deref() == Some("rwy") {
            if let Some((firs, _)) = parse_gng_sector_target(&name) {
                for fir in firs {
                    let dst = sectors_dir.join(format!("{}.rwy", fir.as_str()));
                    plan.ops.push(FileOp::Copy {
                        src: path.to_path_buf(),
                        dst,
                    });
                }
            }
            continue;
        }

        // Everything else from the package is taken ONLY when it is an ICAO or
        // NavData file (under a FIR or LFXX) — those are the paths in the
        // GNG-owned list. `.prf`, `.rwy`, settings, plugins, copyright, etc. are
        // GitHub-provided and intentionally ignored here.
        if let Some(rel) = locate_inside_fir_or_lfxx(gng_root, path) {
            let gng_set = gng_owned_set();
            if is_gng_owned(&gng_set, &rel) {
                plan.ops.push(FileOp::Copy {
                    src: path.to_path_buf(),
                    dst: install_root.join(&rel),
                });
                // CoFrance reads ICAO/NavData from LFXX, so mirror a FIR's copy
                // there as well.
                if let Some(lffx_rel) = mirror_navdata_to_lfxx(&rel) {
                    plan.ops.push(FileOp::Copy {
                        src: path.to_path_buf(),
                        dst: install_root.join(lffx_rel),
                    });
                }
            }
        }
    }
}

/// Returns the destination-relative path for a GNG file, anchored at the
/// first FIR/LFXX segment encountered in the package. Returns `None` if no
/// such segment exists.
fn locate_inside_fir_or_lfxx(gng_root: &Path, path: &Path) -> Option<PathBuf> {
    let rel = path.strip_prefix(gng_root).ok()?;
    let parts: Vec<_> = rel.iter().collect();
    for (idx, part) in parts.iter().enumerate() {
        let s = part.to_string_lossy();
        let upper = s.to_ascii_uppercase();
        if upper == "LFXX" || FirCode::ALL.iter().any(|fir| fir.as_str() == upper.as_str()) {
            let tail: PathBuf = parts[idx..].iter().collect();
            return Some(tail);
        }
        if upper == "LFFM" {
            // Legacy: rewrite LFFM into LFXX.
            let mut p = PathBuf::from("LFXX");
            for tail_part in &parts[idx + 1..] {
                p.push(tail_part);
            }
            return Some(p);
        }
    }
    None
}

fn plan_sector_backups(
    install_root: &Path,
    sector_targets: &BTreeSet<(FirCode, SectorExt)>,
    previous_airac: Option<&str>,
    plan: &mut SyncPlan,
) {
    let sectors_dir = install_root.join(SECTORS_SUBPATH);
    let backup_dir = sectors_dir.join(SECTOR_BACKUP_DIRNAME);

    plan.ops.insert(0, FileOp::EnsureDir {
        path: backup_dir.clone(),
    });

    for (fir, ext) in sector_targets {
        let existing = sectors_dir.join(format!("{}.{}", fir.as_str(), ext.as_str()));
        if existing.exists() {
            let cycle = previous_airac.unwrap_or("unknown");
            let mut backup = backup_dir.join(format!("{}-{}.{}", fir.as_str(), cycle, ext.as_str()));
            if backup.exists() {
                let ts = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
                backup = backup_dir.join(format!(
                    "{}-{}_{}.{}",
                    fir.as_str(),
                    cycle,
                    ts,
                    ext.as_str()
                ));
            }
            plan.ops.push(FileOp::BackupSector {
                src: existing,
                dst: backup,
            });
        }
    }
}

fn read_current_airac(install_root: &Path) -> Option<String> {
    let path = install_root.join(CURRENT_AIRAC_FILE);
    let content = std::fs::read_to_string(&path).ok()?;
    let trimmed = content.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

// Convert a SyncPlan into a result summary after apply.
pub fn summarize(plan: &SyncPlan, written: usize, skipped: usize) -> SyncSummary {
    SyncSummary {
        github_sha: plan.github_short_sha.clone(),
        airac_cycle: plan.detected_airac.clone(),
        files_written: written,
        files_skipped: skipped,
        warnings: plan.warnings.clone(),
    }
}

