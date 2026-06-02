use crate::github;
use crate::profile_store;
use anyhow::Context;
use controller_pack_core::pack_sync::{apply, plan, AreaSource, PlanInputs, SyncSummary};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};

#[derive(serde::Serialize, Clone)]
struct Progress {
    step: String,
}

fn emit(app: &AppHandle, step: &str) {
    let _ = app.emit("sync:progress", Progress { step: step.into() });
}

/// Diagnostic notes go to debug; only real warnings are logged as warnings.
fn log_outcome(notes: &[String], warnings: &[String]) {
    for note in notes {
        tracing::debug!("{note}");
    }
    if !warnings.is_empty() {
        tracing::warn!(count = warnings.len(), "sync produced warnings:");
        for w in warnings {
            tracing::warn!("  • {w}");
        }
    }
}

/// Persist installed versions from the summary and optionally re-apply the
/// VATSIM profile to the pack. Shared by both sync entry points.
async fn finalize(
    app: &AppHandle,
    install_root: &Path,
    also_apply_profile: Option<bool>,
    summary: SyncSummary,
) -> anyhow::Result<SyncSummary> {
    let mut profile = profile_store::load(app)?;
    if let Some(sha) = &summary.github_sha {
        profile.versions.installed_github_sha = Some(sha.clone());
    }
    if let Some(cycle) = &summary.airac_cycle {
        profile.versions.installed_airac_cycle = Some(cycle.clone());
    }
    profile_store::save(app, &profile)?;

    let apply_profile = also_apply_profile.unwrap_or(profile.preferences.apply_creds_after_sync);
    if apply_profile && !profile.vatsim.cid.is_empty() {
        emit(app, "Applying profile credentials");
        controller_pack_core::profile_configurator::apply(install_root, &profile)?;
    }

    emit(app, "Done");
    Ok(summary)
}

pub async fn run_sync(
    app: &AppHandle,
    package_paths: Vec<PathBuf>,
    also_apply_profile: Option<bool>,
) -> anyhow::Result<SyncSummary> {
    let profile = profile_store::load(app)?;
    let install_root = profile
        .controller_pack_dir
        .clone()
        .context("controller pack directory is not set")?;

    emit(app, "Resolving latest GitHub revision");
    let github_short_sha = github::get_short_sha().await.ok();

    emit(app, "Downloading GitHub repository");
    let github_repo = github::download_repo()
        .await
        .context("failed to download GitHub repo")?;

    emit(app, "Extracting selected packages");
    let mut packages = Vec::with_capacity(package_paths.len());
    for path in &package_paths {
        packages.push(
            crate::local_packages::extract_package(path)
                .with_context(|| format!("extracting {}", path.display()))?,
        );
    }
    let pack_roots: Vec<PathBuf> = packages.iter().map(|p| p.root().to_path_buf()).collect();

    emit(app, "Planning file operations");
    let plan_result = plan(PlanInputs {
        github_root: Some(github_repo.root()),
        gng_roots: &pack_roots,
        install_root: &install_root,
        github_short_sha: github_short_sha.clone(),
        area_source: AreaSource::Packages,
    })?;

    emit(app, "Applying changes");
    let summary = apply(&install_root, &plan_result)?;
    log_outcome(&plan_result.notes, &summary.warnings);

    finalize(app, &install_root, also_apply_profile, summary).await
}

/// Refresh the GitHub-managed files for the FIRs already installed, without
/// re-supplying the packages. Does not touch sectors/AIRAC or remove anything;
/// it backs up `LFXX/Settings` and re-lays the GitHub overlay in place.
pub async fn update_from_github(
    app: &AppHandle,
    also_apply_profile: Option<bool>,
) -> anyhow::Result<SyncSummary> {
    let profile = profile_store::load(app)?;
    let install_root = profile
        .controller_pack_dir
        .clone()
        .context("controller pack directory is not set")?;

    emit(app, "Resolving latest GitHub revision");
    let github_short_sha = github::get_short_sha().await.ok();

    emit(app, "Downloading GitHub repository");
    let github_repo = github::download_repo()
        .await
        .context("failed to download GitHub repo")?;

    emit(app, "Updating files from GitHub");
    let plan_result = plan(PlanInputs {
        github_root: Some(github_repo.root()),
        gng_roots: &[],
        install_root: &install_root,
        github_short_sha: github_short_sha.clone(),
        area_source: AreaSource::InstalledOnly,
    })?;

    emit(app, "Applying changes");
    let summary = apply(&install_root, &plan_result)?;
    log_outcome(&plan_result.notes, &summary.warnings);

    finalize(app, &install_root, also_apply_profile, summary).await
}
