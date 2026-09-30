//! Where vATIS keeps its data, and how to tell whether it is installed.
//!
//! vATIS derives everything from one directory:
//!
//! ```csharp
//! _appDataPath = Path.Combine(
//!     Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
//!     "org.vatsim.vatis");
//! ```
//!
//! `LocalApplicationData` is not the same idea on every platform. On Windows it
//! is `%LOCALAPPDATA%`. On macOS .NET routes it to `NSApplicationSupportDirectory`
//! — `~/Library/Application Support` — *not* the XDG `~/.local/share` that the
//! generic Unix branch gives Linux. Getting that wrong would silently write
//! profiles somewhere vATIS never reads.
//!
//! vATIS ships natively for all three, so unlike EuroScope it is never looked
//! for inside a Wine prefix.
//!
//! The platform, the home directory and `$XDG_DATA_HOME` are all parameters
//! rather than `cfg!` conditions or environment reads, so each platform's layout
//! is exercised by the test suite on any host. That matters here: the macOS and
//! Linux branches are the ones that cannot be checked by hand, so they need to be
//! the ones the tests cover unconditionally.

use std::path::{Path, PathBuf};

/// The Velopack pack id, which is also the name of vATIS's data directory.
pub const APP_ID: &str = "org.vatsim.vatis";

/// A platform vATIS ships for: a Windows installer, a macOS disk image and a
/// Linux AppImage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Windows,
    MacOs,
    Linux,
}

impl Platform {
    /// The platform this build is running on.
    pub fn host() -> Self {
        if cfg!(windows) {
            Platform::Windows
        } else if cfg!(target_os = "macos") {
            Platform::MacOs
        } else {
            Platform::Linux
        }
    }
}

/// `$XDG_DATA_HOME` from the environment.
pub fn xdg_data_home() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME").map(PathBuf::from)
}

/// `LocalApplicationData` as .NET resolves it, for an explicit platform.
///
/// Only Linux reads `xdg_data_home`, and only when it is rooted: .NET's Unix
/// branch ignores a relative value and falls back to `~/.local/share`.
fn local_app_data_for(platform: Platform, home: &Path, xdg_data_home: Option<&Path>) -> PathBuf {
    match platform {
        Platform::Windows => home.join("AppData").join("Local"),
        Platform::MacOs => home.join("Library").join("Application Support"),
        Platform::Linux => match xdg_data_home {
            Some(dir) if dir.has_root() => dir.to_path_buf(),
            _ => home.join(".local").join("share"),
        },
    }
}

/// vATIS's application data directory, for an explicit platform.
///
/// On Windows this is also the Velopack install root, so the client binary and
/// the user's profiles share a folder.
pub fn app_data_dir_for(platform: Platform, home: &Path, xdg_data_home: Option<&Path>) -> PathBuf {
    local_app_data_for(platform, home, xdg_data_home).join(APP_ID)
}

/// The profile store, for an explicit platform. vATIS reads `*.json` from here,
/// non-recursively.
pub fn profiles_dir_for(platform: Platform, home: &Path, xdg_data_home: Option<&Path>) -> PathBuf {
    app_data_dir_for(platform, home, xdg_data_home).join("Profiles")
}

/// Where a vATIS client may be installed, most likely first.
///
/// Windows installs are Velopack (`--noPortable`), which lays the app down at
/// `<pack id>/current/`. macOS ships a DMG containing a portable bundle
/// (`--noInst`), so it lands wherever the user dragged it — conventionally
/// `/Applications`, but a per-user `~/Applications` is just as valid.
///
/// Linux ships a portable AppImage, which also lives wherever the user put it,
/// and may have been renamed on the way (AppImageLauncher appends a digest). So
/// for Linux these are *folders*, and any `vATIS*.AppImage` in one counts:
/// `~/Applications` is where AppImageLauncher integrates images and where this
/// installer puts one, `~/AppImages` is Gear Lever's, and the rest are where a
/// manual download tends to stay.
pub fn client_probes_for(platform: Platform, home: &Path) -> Vec<PathBuf> {
    match platform {
        Platform::Windows => {
            vec![app_data_dir_for(platform, home, None).join("current").join("vATIS.exe")]
        }
        Platform::MacOs => vec![
            PathBuf::from("/Applications/vATIS.app"),
            home.join("Applications").join("vATIS.app"),
        ],
        Platform::Linux => vec![
            home.join("Applications"),
            home.join("AppImages"),
            home.join(".local").join("bin"),
            home.join("bin"),
            home.join("Downloads"),
            home.join("Desktop"),
        ],
    }
}

/// Where this installer puts the Linux AppImage.
pub fn linux_client_install_path(home: &Path) -> PathBuf {
    home.join("Applications").join("vATIS.AppImage")
}

fn is_vatis_appimage(file_name: &str) -> bool {
    let name = file_name.to_ascii_lowercase();
    name.starts_with("vatis") && name.ends_with(".appimage")
}

/// The first vATIS AppImage in `dir`, in a stable order.
fn find_appimage(dir: &Path) -> Option<PathBuf> {
    let mut images: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|e| is_vatis_appimage(&e.file_name().to_string_lossy()))
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();
    images.sort();
    images.into_iter().next()
}

/// The installed vATIS client for an explicit platform, if one is present.
///
/// Deliberately does *not* accept the application data directory as proof: it
/// outlives an uninstall, and vATIS also points its crash reporter's cache
/// there, so the folder can exist on a machine that has no client at all.
pub fn detect_client_for(platform: Platform, home: &Path) -> Option<PathBuf> {
    let probes = client_probes_for(platform, home);
    match platform {
        Platform::Linux => probes.iter().find_map(|dir| find_appimage(dir)),
        Platform::Windows | Platform::MacOs => probes.into_iter().find(|p| p.exists()),
    }
}

/// vATIS's application data directory on this host.
pub fn app_data_dir(home: &Path) -> PathBuf {
    app_data_dir_for(Platform::host(), home, xdg_data_home().as_deref())
}

/// The profile store on this host.
pub fn profiles_dir(home: &Path) -> PathBuf {
    app_data_dir(home).join("Profiles")
}

/// Where superseded profiles are moved. A subdirectory of the store, which
/// vATIS therefore never enumerates — it globs the top level only.
pub fn backup_dir(home: &Path) -> PathBuf {
    profiles_dir(home).join("backup")
}

/// The file vATIS stores a profile in, named by the profile's own id.
pub fn profile_path(home: &Path, id: &str) -> PathBuf {
    profiles_dir(home).join(format!("{id}.json"))
}

/// The installed vATIS client on this host, if one is present.
pub fn detect_client(home: &Path) -> Option<PathBuf> {
    detect_client_for(Platform::host(), home)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    const ALL: [Platform; 3] = [Platform::Windows, Platform::MacOs, Platform::Linux];

    #[test]
    fn windows_data_directory_is_under_local_appdata() {
        let home = Path::new("/fake/home");
        assert_eq!(
            app_data_dir_for(Platform::Windows, home, None),
            home.join("AppData/Local/org.vatsim.vatis")
        );
    }

    /// .NET's macOS branch resolves LocalApplicationData to Application Support,
    /// never to the XDG data directory. Writing to `.local/share` would put
    /// profiles somewhere vATIS never looks. Runs on every host, because macOS
    /// is a platform that cannot be checked by hand.
    #[test]
    fn macos_profile_store_is_under_application_support_not_xdg() {
        let home = Path::new("/fake/home");
        let profiles = profiles_dir_for(Platform::MacOs, home, Some(Path::new("/fake/xdg")));

        assert_eq!(
            profiles,
            home.join("Library/Application Support/org.vatsim.vatis/Profiles")
        );
        let shown = profiles.to_string_lossy();
        assert!(!shown.contains(".local/share"), "resolved to XDG: {shown}");
        assert!(!shown.contains("xdg"), "honoured XDG_DATA_HOME: {shown}");
        assert!(!shown.contains(".config"), "resolved to XDG config: {shown}");
    }

    /// .NET's generic Unix branch: `~/.local/share`, not `.config` and not the
    /// macOS layout.
    #[test]
    fn linux_profile_store_is_under_local_share() {
        let home = Path::new("/fake/home");
        assert_eq!(
            profiles_dir_for(Platform::Linux, home, None),
            home.join(".local/share/org.vatsim.vatis/Profiles")
        );
    }

    #[test]
    fn linux_honours_a_rooted_xdg_data_home() {
        let home = Path::new("/fake/home");
        let xdg = Path::new("/data/me");
        assert_eq!(
            app_data_dir_for(Platform::Linux, home, Some(xdg)),
            xdg.join("org.vatsim.vatis")
        );
    }

    /// .NET only uses `$XDG_DATA_HOME` when it is rooted; a relative value
    /// falls back to the default rather than resolving against the cwd.
    #[test]
    fn linux_ignores_a_relative_xdg_data_home() {
        let home = Path::new("/fake/home");
        assert_eq!(
            app_data_dir_for(Platform::Linux, home, Some(Path::new("relative/data"))),
            home.join(".local/share/org.vatsim.vatis")
        );
    }

    #[test]
    fn the_platforms_do_not_share_a_layout() {
        let home = Path::new("/fake/home");
        let dirs: Vec<PathBuf> = ALL.iter().map(|p| app_data_dir_for(*p, home, None)).collect();
        assert_ne!(dirs[0], dirs[1]);
        assert_ne!(dirs[0], dirs[2]);
        assert_ne!(dirs[1], dirs[2]);
    }

    #[test]
    fn windows_probes_the_velopack_current_directory() {
        let home = Path::new("/fake/home");
        assert_eq!(
            client_probes_for(Platform::Windows, home),
            vec![home.join("AppData/Local/org.vatsim.vatis/current/vATIS.exe")]
        );
    }

    #[test]
    fn macos_probes_both_applications_folders_in_order() {
        let home = Path::new("/fake/home");
        assert_eq!(
            client_probes_for(Platform::MacOs, home),
            vec![
                PathBuf::from("/Applications/vATIS.app"),
                home.join("Applications/vATIS.app"),
            ]
        );
    }

    /// The installer's own download must land where detection looks first, or a
    /// fresh install would not be recognised on "Check again".
    #[test]
    fn linux_install_path_is_the_first_folder_probed() {
        let home = Path::new("/fake/home");
        let install = linux_client_install_path(home);
        let probes = client_probes_for(Platform::Linux, home);
        assert_eq!(install.parent(), probes.first().map(PathBuf::as_path));
        assert!(is_vatis_appimage(&install.file_name().unwrap().to_string_lossy()));
    }

    #[test]
    fn linux_finds_a_renamed_appimage() {
        let tmp = tempdir().unwrap();
        let home = tmp.path();
        let image = home.join("Applications/vATIS_4f2a9c.AppImage");
        fs::create_dir_all(image.parent().unwrap()).unwrap();
        fs::write(&image, b"elf").unwrap();

        assert_eq!(detect_client_for(Platform::Linux, home), Some(image));
    }

    #[test]
    fn linux_ignores_other_appimages_and_partial_downloads() {
        let tmp = tempdir().unwrap();
        let home = tmp.path();
        let dir = home.join("Applications");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("Other.AppImage"), b"elf").unwrap();
        fs::write(dir.join("vATIS.AppImage.part"), b"half").unwrap();
        fs::create_dir_all(dir.join("vATIS.AppImage")).unwrap();

        assert_eq!(detect_client_for(Platform::Linux, home), None);
    }

    #[test]
    fn linux_prefers_earlier_folders() {
        let tmp = tempdir().unwrap();
        let home = tmp.path();
        let downloaded = home.join("Downloads/vATIS.AppImage");
        let integrated = home.join("Applications/vATIS.AppImage");
        for image in [&downloaded, &integrated] {
            fs::create_dir_all(image.parent().unwrap()).unwrap();
            fs::write(image, b"elf").unwrap();
        }

        assert_eq!(detect_client_for(Platform::Linux, home), Some(integrated));
    }

    #[test]
    fn backup_is_a_subdirectory_of_the_store() {
        let home = Path::new("/fake/home");
        assert_eq!(backup_dir(home), profiles_dir(home).join("backup"));
    }

    #[test]
    fn profile_is_named_by_its_id() {
        let home = Path::new("/fake/home");
        assert_eq!(
            profile_path(home, "47f4bce0-29f8-4f3f-ae20-a6255b861f88"),
            profiles_dir(home).join("47f4bce0-29f8-4f3f-ae20-a6255b861f88.json"),
        );
    }

    #[test]
    fn client_detected_at_its_platform_location() {
        let tmp = tempdir().unwrap();
        let home = tmp.path();
        assert_eq!(detect_client(home), None);

        // `/Applications` is not writable from a test, so on macOS hosts this
        // exercises the per-user fallback; the ordering itself is asserted
        // separately by `macos_probes_both_applications_folders_in_order`.
        let probe = match Platform::host() {
            Platform::Windows => app_data_dir(home).join("current").join("vATIS.exe"),
            Platform::MacOs => home.join("Applications").join("vATIS.app"),
            Platform::Linux => linux_client_install_path(home),
        };
        fs::create_dir_all(probe.parent().unwrap()).unwrap();
        fs::write(&probe, b"client").unwrap();

        assert_eq!(detect_client(home), Some(probe));
    }

    /// The data directory survives an uninstall and doubles as a crash-reporter
    /// cache, so on its own it proves nothing.
    #[test]
    fn app_data_directory_alone_is_not_an_installed_client() {
        let tmp = tempdir().unwrap();
        let home = tmp.path();
        // macOS probes `/Applications`, outside the fake home, so a Mac with
        // vATIS installed would fail this for reasons unrelated to the data
        // directory.
        let platforms = [Platform::Windows, Platform::Linux];

        for platform in platforms {
            // Explicitly no XDG_DATA_HOME: the host's must not leak into a test.
            let data = app_data_dir_for(platform, home, None);
            fs::create_dir_all(data.join("Profiles")).unwrap();
            fs::write(data.join("AppConfig.json"), b"{}").unwrap();
        }

        for platform in platforms {
            assert_eq!(detect_client_for(platform, home), None, "{platform:?}");
        }
    }

    #[test]
    fn paths_are_relative_to_the_home_they_are_given() {
        for platform in ALL {
            let a = app_data_dir_for(platform, Path::new("/home/one"), None);
            let b = app_data_dir_for(platform, Path::new("/home/two"), None);
            assert_ne!(a, b);
            assert!(a.starts_with("/home/one"));
        }
    }
}
