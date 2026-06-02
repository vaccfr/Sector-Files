use crate::github;
use crate::profile_store::{self, Profile};
use serde::Serialize;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CheckStatus {
    UpToDate,
    UpdateAvailable { value: String },
    Unknown { reason: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct CheckUpdatesReport {
    pub github: CheckStatus,
    pub airac: CheckStatus,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstallerUpdateReport {
    pub available: bool,
    pub latest_version: Option<String>,
    pub current_version: String,
}

pub async fn check_updates(app: &AppHandle) -> anyhow::Result<CheckUpdatesReport> {
    let profile = profile_store::load(app)?;
    let github = check_github(&profile).await;
    let airac = check_airac(&profile).await;
    let report = CheckUpdatesReport { github, airac };
    app.emit("updates:report", &report).ok();
    Ok(report)
}

async fn check_github(profile: &Profile) -> CheckStatus {
    match github::get_short_sha().await {
        Ok(sha) => match profile.versions.installed_github_sha.as_deref() {
            Some(installed) if installed == sha => CheckStatus::UpToDate,
            _ => CheckStatus::UpdateAvailable { value: sha },
        },
        Err(e) => CheckStatus::Unknown {
            reason: e.to_string(),
        },
    }
}

async fn check_airac(_profile: &Profile) -> CheckStatus {
    // AIRAC packages are now selected manually by the user, so there's no
    // remote "latest cycle" endpoint to compare against. We surface the
    // currently-installed cycle in the UI instead.
    CheckStatus::Unknown {
        reason: "AIRAC packages are selected manually".into(),
    }
}

pub async fn check_installer_update(_app: &AppHandle) -> anyhow::Result<InstallerUpdateReport> {
    // The actual update bundle download + signature verification is handled by
    // `tauri-plugin-updater`. This command is a thin wrapper that returns
    // metadata so the UI can show a banner; the plugin itself drives the
    // install step from the frontend.
    Ok(InstallerUpdateReport {
        available: false,
        latest_version: None,
        current_version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// Background task: periodically check for content updates while the app
/// window is open. Backoff is 30 minutes between automatic checks; the user
/// can always trigger an immediate check from the UI.
pub async fn spawn_background_checker(app: AppHandle) {
    // Initial check on launch (run after a short delay so the window has rendered).
    tokio::time::sleep(Duration::from_secs(3)).await;
    let _ = check_updates(&app).await;

    loop {
        tokio::time::sleep(Duration::from_secs(30 * 60)).await;
        let profile = match profile_store::load(&app) {
            Ok(p) => p,
            Err(_) => continue,
        };
        if !profile.preferences.auto_check_updates {
            continue;
        }
        let _ = check_updates(&app).await;
    }
}
