//! Finding the Wine prefixes EuroScope may live in.
//!
//! EuroScope is Windows-only, so on macOS and Linux it runs inside a Wine
//! prefix — a directory holding a `drive_c` — that is usually created and named
//! by a front-end (CrossOver, Whisky, Bottles, Lutris…) rather than by the user,
//! and tucked away in a hidden folder. The controller pack belongs next to
//! EuroScope, so this module enumerates the prefixes every common front-end
//! keeps and says which of them actually have EuroScope installed.
//!
//! The locations are the union of both platforms'. A macOS path never exists on
//! Linux and vice versa, so scanning both costs a few failed `read_dir`s and
//! lets the whole table be tested on any host. Only [`detect_prefixes`] reads
//! the environment.

use serde::Serialize;
use std::path::{Path, PathBuf};

/// Folder a fresh pack is proposed in, under a Windows user's Documents.
pub const PACK_FOLDER: &str = "EuroScope";

/// A Wine prefix found on disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WinePrefix {
    /// The prefix itself: the directory holding `drive_c`.
    pub path: PathBuf,
    /// The front-end that manages it, for display ("CrossOver", "Bottles"…).
    pub manager: &'static str,
    /// The bottle's own name, for display.
    pub name: String,
    /// Whether EuroScope is installed in it.
    pub has_euroscope: bool,
    /// The Documents folder of the prefix's Windows user, if it has one.
    pub documents_dir: Option<PathBuf>,
    /// Where a fresh controller pack would go in this prefix.
    pub suggested_pack_dir: Option<PathBuf>,
}

/// A directory holding one prefix per child.
struct Collection {
    manager: &'static str,
    root: PathBuf,
    /// Where the prefix sits inside each child; `""` is the child itself.
    inner: &'static [&'static str],
    /// Wrapper apps: only `.app` children count, and are named without it.
    apps_only: bool,
}

fn collection(manager: &'static str, root: PathBuf, inner: &'static [&'static str]) -> Collection {
    Collection { manager, root, inner, apps_only: false }
}

/// A macOS wrapper keeps its prefix inside the bundle: `SharedSupport/prefix`
/// today, `Resources` in older Wineskin builds.
fn wrappers(manager: &'static str, root: PathBuf) -> Collection {
    Collection {
        manager,
        root,
        inner: &["Contents/SharedSupport/prefix", "Contents/Resources"],
        apps_only: true,
    }
}

fn collections(home: &Path) -> Vec<Collection> {
    vec![
        // macOS
        collection("CrossOver", home.join("Library/Application Support/CrossOver/Bottles"), &[""]),
        collection("Whisky", home.join("Library/Containers/com.isaacmarovitz.Whisky/Bottles"), &[""]),
        wrappers("Kegworks", home.join("Applications/Kegworks")),
        wrappers("Sikarugir", home.join("Applications/Sikarugir")),
        wrappers("Wineskin", home.join("Applications/Wineskin")),
        // Porting Kit and hand-made wrappers land straight in ~/Applications.
        wrappers("Wine wrapper", home.join("Applications")),
        // Linux
        collection("Bottles", home.join(".local/share/bottles/bottles"), &[""]),
        collection(
            "Bottles",
            home.join(".var/app/com.usebottles.bottles/data/bottles/bottles"),
            &[""],
        ),
        // Lutris install scripts use the game folder as the prefix, or a
        // `prefix` folder inside it.
        collection("Lutris", home.join("Games"), &["", "prefix"]),
        collection("PlayOnLinux", home.join(".PlayOnLinux/wineprefix"), &[""]),
        collection("CrossOver", home.join(".cxoffice"), &[""]),
        // Either: the winetricks convention.
        collection("Wine", home.join(".local/share/wineprefixes"), &[""]),
    ]
}

fn is_prefix(path: &Path) -> bool {
    path.join("drive_c").is_dir()
}

fn has_euroscope(prefix: &Path) -> bool {
    let drive_c = prefix.join("drive_c");
    ["Program Files (x86)", "Program Files"]
        .iter()
        .any(|pf| drive_c.join(pf).join("EuroScope").join("EuroScope.exe").is_file())
}

/// Every Windows user profile in a prefix, `Public` excepted, in a stable order.
pub fn user_dirs(prefix: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(prefix.join("drive_c").join("users")) else {
        return Vec::new();
    };
    let mut users: Vec<PathBuf> = entries
        .flatten()
        .filter(|e| !e.file_name().to_string_lossy().eq_ignore_ascii_case("Public"))
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    users.sort();
    users
}

/// A user's Documents folder under both names Wine has used for it.
fn documents_candidates(user: &Path) -> [PathBuf; 2] {
    [user.join("Documents"), user.join("My Documents")]
}

/// Every place a pack in this prefix is likely to sit under.
pub fn document_dirs(prefix: &Path) -> Vec<PathBuf> {
    user_dirs(prefix).iter().flat_map(|u| documents_candidates(u)).collect()
}

/// The prefix user's Documents folder: the first one that exists, else where
/// current Wine would put it.
fn documents_dir(prefix: &Path) -> Option<PathBuf> {
    let users = user_dirs(prefix);
    users
        .iter()
        .flat_map(|u| documents_candidates(u))
        .find(|d| d.is_dir())
        .or_else(|| users.first().map(|u| u.join("Documents")))
}

fn describe(path: PathBuf, manager: &'static str, name: String) -> WinePrefix {
    let documents_dir = documents_dir(&path);
    WinePrefix {
        has_euroscope: has_euroscope(&path),
        suggested_pack_dir: documents_dir.as_ref().map(|d| d.join(PACK_FOLDER)),
        documents_dir,
        path,
        manager,
        name,
    }
}

fn file_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Every Wine prefix under `home`, prefixes with EuroScope installed first and
/// discovery order otherwise. `env_prefix` is `$WINEPREFIX`, which comes first
/// among equals: a user who set it meant it.
pub fn find_prefixes(home: &Path, env_prefix: Option<&Path>) -> Vec<WinePrefix> {
    let mut found: Vec<WinePrefix> = Vec::new();
    let mut add = |path: PathBuf, manager: &'static str, name: String| {
        if is_prefix(&path) && !found.iter().any(|p| p.path == path) {
            found.push(describe(path, manager, name));
        }
    };

    if let Some(prefix) = env_prefix.filter(|p| p.has_root()) {
        add(prefix.to_path_buf(), "Wine", file_name(prefix));
    }
    add(home.join(".wine"), "Wine", ".wine".to_string());

    for c in collections(home) {
        let Ok(entries) = std::fs::read_dir(&c.root) else {
            continue;
        };
        let mut children: Vec<PathBuf> =
            entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
        // Stable order so the same prefix is preferred every run.
        children.sort();
        for child in children {
            let is_app = child.extension().is_some_and(|e| e.eq_ignore_ascii_case("app"));
            if c.apps_only != is_app {
                continue;
            }
            let name = if c.apps_only {
                child.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
            } else {
                file_name(&child)
            };
            for inner in c.inner {
                let path = if inner.is_empty() { child.clone() } else { child.join(inner) };
                add(path, c.manager, name.clone());
            }
        }
    }

    // Stable: discovery order survives within each group.
    found.sort_by_key(|p| !p.has_euroscope);
    found
}

/// [`find_prefixes`] against the real home directory and `$WINEPREFIX`. Always
/// empty on Windows, where EuroScope runs natively.
pub fn detect_prefixes() -> Vec<WinePrefix> {
    if cfg!(windows) {
        return Vec::new();
    }
    let Some(home) = crate::pack_dir::home_dir() else {
        return Vec::new();
    };
    let env_prefix = std::env::var_os("WINEPREFIX").map(PathBuf::from);
    find_prefixes(&home, env_prefix.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    /// A booted prefix with one Windows user.
    fn make_prefix(prefix: &Path, user: &str) {
        fs::create_dir_all(prefix.join("drive_c/users").join(user).join("Documents")).unwrap();
        fs::create_dir_all(prefix.join("drive_c/users/Public")).unwrap();
    }

    fn install_euroscope(prefix: &Path) {
        let dir = prefix.join("drive_c/Program Files (x86)/EuroScope");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("EuroScope.exe"), b"exe").unwrap();
    }

    fn paths(prefixes: &[WinePrefix]) -> Vec<PathBuf> {
        prefixes.iter().map(|p| p.path.clone()).collect()
    }

    #[test]
    fn every_front_end_is_found() {
        let tmp = tempdir().unwrap();
        let home = tmp.path();
        let expected = [
            ("Wine", home.join(".wine")),
            ("CrossOver", home.join("Library/Application Support/CrossOver/Bottles/ES")),
            ("Whisky", home.join("Library/Containers/com.isaacmarovitz.Whisky/Bottles/ES")),
            ("Kegworks", home.join("Applications/Kegworks/ES.app/Contents/SharedSupport/prefix")),
            ("Wineskin", home.join("Applications/Wineskin/Old.app/Contents/Resources")),
            ("Wine wrapper", home.join("Applications/Port.app/Contents/SharedSupport/prefix")),
            ("Bottles", home.join(".local/share/bottles/bottles/ES")),
            ("Bottles", home.join(".var/app/com.usebottles.bottles/data/bottles/bottles/ES")),
            ("Lutris", home.join("Games/euroscope")),
            ("Lutris", home.join("Games/other/prefix")),
            ("PlayOnLinux", home.join(".PlayOnLinux/wineprefix/ES")),
            ("CrossOver", home.join(".cxoffice/ES")),
            ("Wine", home.join(".local/share/wineprefixes/ES")),
        ];
        for (_, prefix) in &expected {
            make_prefix(prefix, "me");
        }

        let found = find_prefixes(home, None);
        for (manager, prefix) in &expected {
            let hit = found.iter().find(|p| &p.path == prefix);
            assert!(hit.is_some(), "{manager} prefix {} not found", prefix.display());
            assert_eq!(hit.unwrap().manager, *manager);
        }
        assert_eq!(found.len(), expected.len());
    }

    #[test]
    fn directories_without_drive_c_are_not_prefixes() {
        let tmp = tempdir().unwrap();
        let home = tmp.path();
        fs::create_dir_all(home.join("Library/Application Support/CrossOver/Bottles/Empty")).unwrap();
        fs::create_dir_all(home.join("Games/native-game")).unwrap();
        fs::create_dir_all(home.join(".wine")).unwrap();
        assert!(find_prefixes(home, None).is_empty());
    }

    #[test]
    fn wrappers_are_named_after_their_app() {
        let tmp = tempdir().unwrap();
        let home = tmp.path();
        make_prefix(&home.join("Applications/Kegworks/EuroScope.app/Contents/SharedSupport/prefix"), "me");
        // Not an app bundle, so not a wrapper, however prefix-like.
        make_prefix(&home.join("Applications/Kegworks/stray"), "me");

        let found = find_prefixes(home, None);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "EuroScope");
    }

    #[test]
    fn prefixes_with_euroscope_come_first() {
        let tmp = tempdir().unwrap();
        let home = tmp.path();
        let plain = home.join(".wine");
        let bottle = home.join(".local/share/bottles/bottles/ATC");
        make_prefix(&plain, "me");
        make_prefix(&bottle, "me");
        install_euroscope(&bottle);

        let found = find_prefixes(home, None);
        assert_eq!(paths(&found), vec![bottle, plain]);
        assert!(found[0].has_euroscope);
        assert!(!found[1].has_euroscope);
    }

    #[test]
    fn wineprefix_is_first_among_equals_and_not_listed_twice() {
        let tmp = tempdir().unwrap();
        let home = tmp.path();
        let custom = home.join("wine/euroscope");
        make_prefix(&home.join(".wine"), "me");
        make_prefix(&custom, "me");

        let found = find_prefixes(home, Some(&custom));
        assert_eq!(paths(&found), vec![custom, home.join(".wine")]);

        // Pointing it at a prefix that is found anyway changes nothing.
        let again = find_prefixes(home, Some(&home.join(".wine")));
        assert_eq!(paths(&again), vec![home.join(".wine")]);
    }

    #[test]
    fn a_relative_wineprefix_is_ignored() {
        let tmp = tempdir().unwrap();
        assert!(find_prefixes(tmp.path(), Some(Path::new("relative/prefix"))).is_empty());
    }

    #[test]
    fn a_fresh_pack_is_proposed_in_the_users_documents() {
        let tmp = tempdir().unwrap();
        let prefix = tmp.path().join(".wine");
        make_prefix(&prefix, "crossover");

        let found = find_prefixes(tmp.path(), None);
        let documents = prefix.join("drive_c/users/crossover/Documents");
        assert_eq!(found[0].documents_dir, Some(documents.clone()));
        assert_eq!(found[0].suggested_pack_dir, Some(documents.join(PACK_FOLDER)));
    }

    #[test]
    fn older_prefixes_use_my_documents() {
        let tmp = tempdir().unwrap();
        let prefix = tmp.path().join(".wine");
        let documents = prefix.join("drive_c/users/me/My Documents");
        fs::create_dir_all(&documents).unwrap();

        assert_eq!(find_prefixes(tmp.path(), None)[0].documents_dir, Some(documents));
    }

    #[test]
    fn document_dirs_skip_the_public_profile() {
        let tmp = tempdir().unwrap();
        let prefix = tmp.path().join("prefix");
        make_prefix(&prefix, "me");

        let users = prefix.join("drive_c/users");
        assert_eq!(
            document_dirs(&prefix),
            vec![users.join("me/Documents"), users.join("me/My Documents")]
        );
    }

    #[test]
    fn a_prefix_with_no_users_proposes_nothing() {
        let tmp = tempdir().unwrap();
        fs::create_dir_all(tmp.path().join(".wine/drive_c")).unwrap();

        let found = find_prefixes(tmp.path(), None);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].suggested_pack_dir, None);
    }
}
