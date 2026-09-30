//! Recognising a controller-pack install root, and guessing where it lives.
//!
//! EuroScope itself is Windows-only, so on macOS and Linux the pack is either a
//! plain folder the user syncs by hand or — far more often — a folder inside a
//! Wine prefix's `drive_c`. The detection below therefore looks at the usual
//! native locations *and* at the `Documents` folder of every Windows user inside
//! every prefix [`crate::wine`] can find.
//!
//! Everything here takes its roots as arguments so it can be unit-tested
//! against a fake home directory; [`detect_pack_dir`] is the thin wrapper that
//! reads the real environment.

use crate::fir::AreaCode;
use crate::wine::{self, WinePrefix};
use std::path::{Path, PathBuf};

/// A directory is a controller pack when it has `LFXX` plus at least one area
/// folder. Any area counts, `LFFM` included — it is not a FIR, but a pack that
/// only installed the military area is still a controller pack.
pub fn looks_like_controller_pack(path: &Path) -> bool {
    let has_lfxx = path.join("LFXX").is_dir();
    let has_any_area = AreaCode::ALL
        .iter()
        .any(|area| path.join(area.as_str()).is_dir());
    has_lfxx && has_any_area
}

/// Directories worth scanning for a controller pack, most likely first. Each is
/// checked both as a pack itself and as a parent holding one (e.g.
/// `Documents/EuroScope`), so no folder *names* need to be guessed.
pub fn pack_dir_search_roots(home: &Path, prefixes: &[WinePrefix]) -> Vec<PathBuf> {
    let mut roots = vec![
        home.join("Documents"),
        home.join("Desktop"),
        home.to_path_buf(),
    ];
    roots.extend(prefixes.iter().flat_map(|p| wine::document_dirs(&p.path)));
    roots
}

/// First directory that looks like a controller pack: the current directory (the
/// legacy layout put the installer *inside* the pack), then each search root and
/// its immediate children. `prefixes` are the Wine prefixes to look inside, in
/// preference order.
pub fn detect_pack_dir_in(
    cwd: Option<&Path>,
    home: Option<&Path>,
    prefixes: &[WinePrefix],
) -> Option<PathBuf> {
    if let Some(cwd) = cwd {
        if looks_like_controller_pack(cwd) {
            return Some(cwd.to_path_buf());
        }
    }
    let home = home?;
    for root in pack_dir_search_roots(home, prefixes) {
        if looks_like_controller_pack(&root) {
            return Some(root);
        }
        let Ok(entries) = std::fs::read_dir(&root) else {
            continue;
        };
        let mut children: Vec<PathBuf> = entries
            .flatten()
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .map(|e| e.path())
            .collect();
        // Stable order so the same install is picked every run.
        children.sort();
        if let Some(found) = children.into_iter().find(|c| looks_like_controller_pack(c)) {
            return Some(found);
        }
    }
    None
}

/// The user's home directory, from the environment.
pub fn home_dir() -> Option<PathBuf> {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(var).map(PathBuf::from).filter(|p| !p.as_os_str().is_empty())
}

/// [`detect_pack_dir_in`] against the real current directory, home and Wine
/// prefixes.
pub fn detect_pack_dir() -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok();
    detect_pack_dir_in(cwd.as_deref(), home_dir().as_deref(), &wine::detect_prefixes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wine::find_prefixes;
    use std::fs;
    use tempfile::tempdir;

    fn make_pack(root: &Path) {
        fs::create_dir_all(root.join("LFXX")).unwrap();
        fs::create_dir_all(root.join("LFBB")).unwrap();
    }

    #[test]
    fn looks_like_controller_pack_requires_lfxx_and_one_area() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        assert!(!looks_like_controller_pack(root));
        fs::create_dir_all(root.join("LFXX")).unwrap();
        assert!(!looks_like_controller_pack(root));
        fs::create_dir_all(root.join("LFBB")).unwrap();
        assert!(looks_like_controller_pack(root));
    }

    #[test]
    fn lffm_only_pack_is_recognised() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("LFXX")).unwrap();
        fs::create_dir_all(root.join("LFFM")).unwrap();
        assert!(looks_like_controller_pack(root));
    }

    #[test]
    fn search_roots_always_include_documents_desktop_and_home() {
        let home = Path::new("/fake/home");
        let roots = pack_dir_search_roots(home, &[]);
        assert!(roots.contains(&home.join("Documents")));
        assert!(roots.contains(&home.join("Desktop")));
        assert!(roots.contains(&home.to_path_buf()));
    }

    #[test]
    fn current_directory_wins_when_it_is_a_pack() {
        let tmp = tempdir().unwrap();
        let cwd = tmp.path().join("cwd");
        make_pack(&cwd);
        let home = tmp.path().join("home");
        make_pack(&home.join("Documents/EuroScope"));

        assert_eq!(detect_pack_dir_in(Some(&cwd), Some(&home), &[]), Some(cwd));
    }

    #[test]
    fn finds_pack_one_level_under_documents() {
        let tmp = tempdir().unwrap();
        let home = tmp.path();
        let pack = home.join("Documents").join("EuroScope");
        make_pack(&pack);
        fs::create_dir_all(home.join("Documents").join("Unrelated")).unwrap();

        assert_eq!(detect_pack_dir_in(None, Some(home), &[]), Some(pack));
    }

    #[test]
    fn returns_none_when_nothing_looks_like_a_pack() {
        let tmp = tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("Documents/Whatever")).unwrap();
        assert_eq!(detect_pack_dir_in(None, Some(tmp.path()), &[]), None);
    }

    #[test]
    fn finds_pack_inside_a_crossover_bottle() {
        let tmp = tempdir().unwrap();
        let home = tmp.path();
        let pack = home
            .join("Library/Application Support/CrossOver/Bottles/EuroScope")
            .join("drive_c/users/crossover/Documents/CoFrance");
        make_pack(&pack);

        let prefixes = find_prefixes(home, None);
        assert_eq!(detect_pack_dir_in(None, Some(home), &prefixes), Some(pack));
    }

    #[test]
    fn finds_pack_inside_a_bottles_prefix() {
        let tmp = tempdir().unwrap();
        let home = tmp.path();
        let pack = home
            .join(".var/app/com.usebottles.bottles/data/bottles/bottles/EuroScope")
            .join("drive_c/users/me/Documents/EuroScope");
        make_pack(&pack);

        let prefixes = find_prefixes(home, None);
        assert_eq!(detect_pack_dir_in(None, Some(home), &prefixes), Some(pack));
    }

    /// Two prefixes each holding a pack: the one EuroScope is installed in is
    /// the one it will load, so it wins regardless of discovery order.
    #[test]
    fn the_pack_next_to_euroscope_wins() {
        let tmp = tempdir().unwrap();
        let home = tmp.path();
        let stale = home.join(".wine/drive_c/users/me/Documents/EuroScope");
        let bottle = home.join(".local/share/bottles/bottles/ATC");
        let live = bottle.join("drive_c/users/me/Documents/EuroScope");
        make_pack(&stale);
        make_pack(&live);
        let euroscope = bottle.join("drive_c/Program Files (x86)/EuroScope");
        fs::create_dir_all(&euroscope).unwrap();
        fs::write(euroscope.join("EuroScope.exe"), b"exe").unwrap();

        let prefixes = find_prefixes(home, None);
        assert_eq!(detect_pack_dir_in(None, Some(home), &prefixes), Some(live));
    }
}
