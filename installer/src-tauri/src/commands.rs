use crate::profile_store::{self, Profile};
use crate::update_check::{CheckUpdatesReport, InstallerUpdateReport};
use controller_pack_core::pack_sync::SyncSummary;
use serde::Deserialize;
use std::path::PathBuf;
use tauri::AppHandle;

#[derive(Debug, Default, Deserialize)]
pub struct ProfilePatch {
    pub controller_pack_dir: Option<Option<PathBuf>>,
    pub vatsim: Option<profile_store::VatsimCredentials>,
    pub versions: Option<profile_store::InstalledVersions>,
    pub preferences: Option<profile_store::Preferences>,
}

#[tauri::command]
pub fn get_profile(app: AppHandle) -> Result<Profile, String> {
    profile_store::load(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_profile(app: AppHandle, patch: ProfilePatch) -> Result<Profile, String> {
    let mut profile = profile_store::load(&app).map_err(|e| e.to_string())?;
    if let Some(dir) = patch.controller_pack_dir {
        profile.controller_pack_dir = dir;
    }
    if let Some(vatsim) = patch.vatsim {
        profile.vatsim = vatsim;
    }
    if let Some(versions) = patch.versions {
        profile.versions = versions;
    }
    if let Some(preferences) = patch.preferences {
        profile.preferences = preferences;
    }
    profile_store::save(&app, &profile).map_err(|e| e.to_string())?;
    Ok(profile)
}

#[tauri::command]
pub fn detect_pack_dir() -> Option<PathBuf> {
    profile_store::detect_pack_dir()
}

#[tauri::command]
pub fn looks_like_controller_pack(path: PathBuf) -> bool {
    profile_store::looks_like_controller_pack(&path)
}

#[tauri::command]
pub async fn run_sync(
    app: AppHandle,
    package_paths: Vec<PathBuf>,
    also_apply_profile: Option<bool>,
) -> Result<SyncSummary, String> {
    crate::sync_orchestrator::run_sync(&app, package_paths, also_apply_profile)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_from_github(
    app: AppHandle,
    also_apply_profile: Option<bool>,
) -> Result<SyncSummary, String> {
    crate::sync_orchestrator::update_from_github(&app, also_apply_profile)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn apply_profile_to_pack(app: AppHandle, install_root: PathBuf) -> Result<usize, String> {
    let profile = profile_store::load(&app).map_err(|e| e.to_string())?;
    controller_pack_core::profile_configurator::apply(&install_root, &profile)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn import_plugin_lines(install_root: PathBuf, example_prf: PathBuf) -> Result<usize, String> {
    controller_pack_core::profile_configurator::import_plugin_lines(&install_root, &example_prf)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn check_updates(app: AppHandle) -> Result<CheckUpdatesReport, String> {
    crate::update_check::check_updates(&app).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn check_installer_update(app: AppHandle) -> Result<InstallerUpdateReport, String> {
    crate::update_check::check_installer_update(&app).await.map_err(|e| e.to_string())
}
