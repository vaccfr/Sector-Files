//! End-to-end tests for the pack_sync planner + applier.
//!
//! Builds a fake GitHub repo tree and fake package tree(s) inside a tempdir,
//! runs `plan` + `apply`, and asserts the resulting install layout.

use super::plan::{plan, AreaSource, PlanInputs};
use super::*;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn write_file(root: &Path, rel: &str, content: &str) {
    let dst = root.join(rel);
    fs::create_dir_all(dst.parent().unwrap()).unwrap();
    fs::write(dst, content).unwrap();
}

fn list_files(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
    {
        let rel = entry
            .path()
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        out.push(rel);
    }
    out.sort();
    out
}

fn build_fake_github_repo() -> (TempDir, PathBuf) {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().join("vaccfr-Sector-Files-abcdef1");

    // Overlay content for several areas + the shared base. GitHub provides
    // everything except sectors / ICAO / NavData — including .prf, CoFrance,
    // copyright.
    write_file(&root, "LFBB/ASR/Tower.asr", "asr content\n");
    write_file(&root, "LFFF/ASR/Paris.asr", "paris asr\n");
    write_file(&root, "LFEE/ASR/Reims.asr", "reims asr\n"); // FIR with no package
    write_file(&root, "LFFM/Secret.txt", "tier-2 only\n"); // secret, gated on package
    write_file(&root, "LFXX/Settings/Symbology.txt", "sym\n");
    write_file(&root, "LFXX/Plugins/CCAMS/CCAMS.dll", "ccams\n");
    write_file(&root, "LFXX/Plugins/CoFrance/CoFranceLoader.dll", "loader\n");
    write_file(&root, "LFXX/Plugins/CoFrance/CoFrance.ini", "cofrance config\n");
    write_file(&root, "LFBB/aeronav_copyright.txt", "(c) AeroNav\n");
    write_file(&root, "LFFF/EGA Paris.prf", "github prf\n"); // profiles come from GitHub

    // Package-owned paths the overlay must NOT write (sectors + ICAO/NavData).
    write_file(&root, "LFXX/Sectors/STALE.txt", "ought-to-be-skipped\n");
    write_file(&root, "LFBB/ICAO/airports.txt", "GH should not write this\n");

    let path = root.clone();
    (tmp, path)
}

/// A package covering LFBB (sectors + rwy + nav data) and LFFF. It also ships a
/// `.prf` and a copyright file that must be IGNORED (those come from GitHub).
fn build_fake_gng_package() -> (TempDir, PathBuf) {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();

    write_file(&root, "LFBB-Bordeaux-260301-0003.sct", "sct content\n");
    write_file(&root, "LFBB-Bordeaux-260301-0003.ese", "ese content\n");
    write_file(&root, "LFBB-Bordeaux-260301-0003.rwy", "rwy content\n");
    write_file(&root, "LFBB/ICAO/airports.txt", "gng airports\n");
    write_file(&root, "LFBB/NavData/airways.txt", "navdata\n");
    // The one Settings file the package owns; the rest of Settings is GitHub's.
    write_file(&root, "LFBB/Settings/VoiceChannels.txt", "gng voice channels\n");
    write_file(&root, "LFFF/EGA Paris.prf", "zip prf — must be ignored\n");
    write_file(&root, "LFBB/aeronav_copyright.txt", "zip copyright — must be ignored\n");

    (tmp, root)
}

/// The GNG combined "North" package: a single sector file (plus `.ese`/`.rwy`)
/// named with the `LFXXN` combined code that must fan out to BOTH LFFF and LFEE.
/// Uses the real GNG filename shape, with an `_`-prefixed creation timestamp
/// before the `-260501-` AIRAC group.
fn build_fake_lfxxn_package() -> (TempDir, PathBuf) {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();

    write_file(&root, "LFXXN-Paris-Reims_20260605153747-260501-0001.sct", "north sct\n");
    write_file(&root, "LFXXN-Paris-Reims_20260605153747-260501-0001.ese", "north ese\n");
    write_file(&root, "LFXXN-Paris-Reims_20260605153747-260501-0001.rwy", "north rwy\n");

    (tmp, root)
}

fn build_existing_install() -> TempDir {
    let tmp = TempDir::new().unwrap();
    let install_root = tmp.path();

    write_file(install_root, "LFXX/Sectors/LFBB.sct", "old sct\n");
    write_file(install_root, "LFXX/Sectors/LFBB.ese", "old ese\n");
    write_file(install_root, "LFXX/Sectors/current_airac.txt", "2602\n");
    write_file(
        install_root,
        "LFXX/Plugins/CoFrance/user_settings.dat",
        "user state\n",
    );

    tmp
}

#[test]
fn end_to_end_sync_produces_expected_layout() {
    let (_gh_tmp, github_root) = build_fake_github_repo();
    let (_gng_tmp, gng_root) = build_fake_gng_package();
    let install_tmp = build_existing_install();
    let install_root = install_tmp.path();
    // Pre-existing settings that the GitHub overlay will overwrite ("sym").
    write_file(install_root, "LFXX/Settings/Symbology.txt", "old sym\n");

    let gng_roots = vec![gng_root.clone()];
    let plan = plan(PlanInputs {
        github_root: Some(&github_root),
        gng_roots: &gng_roots,
        install_root,
        github_short_sha: Some("abcdef1".into()),
        area_source: AreaSource::Packages,
    })
    .unwrap();

    let summary = apply(install_root, &plan).unwrap();
    let files = list_files(install_root);

    // The old settings were backed up before the overlay overwrote them.
    let backed_up =
        fs::read_to_string(install_root.join("LFXX/Settings/backup/Symbology.txt")).unwrap();
    assert_eq!(backed_up.trim(), "old sym");
    let now = fs::read_to_string(install_root.join("LFXX/Settings/Symbology.txt")).unwrap();
    assert_eq!(now.trim(), "sym");

    // Sector files at LFXX/Sectors with FIR-only names, old ones backed up.
    assert!(files.contains(&"LFXX/Sectors/LFBB.sct".to_string()));
    assert!(files.contains(&"LFXX/Sectors/LFBB.ese".to_string()));
    assert!(
        files.iter().any(|f| f.starts_with("LFXX/Sectors/Backup/LFBB-2602")),
        "expected LFBB-2602 backup; got: {files:#?}"
    );

    // ICAO/NavData come from the package, into the FIR folder...
    let icao = fs::read_to_string(install_root.join("LFBB/ICAO/airports.txt")).unwrap();
    assert_eq!(icao.trim(), "gng airports");
    // ...and are ALSO mirrored into LFXX (CoFrance reads them there).
    let lffx_icao = fs::read_to_string(install_root.join("LFXX/ICAO/airports.txt")).unwrap();
    assert_eq!(lffx_icao.trim(), "gng airports");
    let lffx_nav = fs::read_to_string(install_root.join("LFXX/NavData/airways.txt")).unwrap();
    assert_eq!(lffx_nav.trim(), "navdata");

    // .rwy is kept from the package, paired with the sector dir.
    let rwy = fs::read_to_string(install_root.join("LFXX/Sectors/LFBB.rwy")).unwrap();
    assert_eq!(rwy.trim(), "rwy content");

    // VoiceChannels.txt is the one package-owned Settings file: it lands in the
    // FIR's own Settings folder and is NOT mirrored into LFXX.
    let voice = fs::read_to_string(install_root.join("LFBB/Settings/VoiceChannels.txt")).unwrap();
    assert_eq!(voice.trim(), "gng voice channels");
    assert!(
        !files.contains(&"LFXX/Settings/VoiceChannels.txt".to_string()),
        "VoiceChannels must not be mirrored into LFXX; got: {files:#?}"
    );

    // GitHub overlay only for covered areas: LFBB + LFFF (package) and LFXX.
    assert!(files.contains(&"LFBB/ASR/Tower.asr".to_string()));
    assert!(files.contains(&"LFFF/ASR/Paris.asr".to_string()));
    assert!(files.contains(&"LFXX/Settings/Symbology.txt".to_string()));

    // No package for LFEE → its GitHub folder is not installed.
    assert!(!files.iter().any(|f| f.starts_with("LFEE/")), "got: {files:#?}");
    // No LFFM package → the secret folder is not installed.
    assert!(!files.iter().any(|f| f.starts_with("LFFM/")), "got: {files:#?}");

    // CoFrance now comes entirely from GitHub.
    assert_eq!(
        fs::read_to_string(install_root.join("LFXX/Plugins/CoFrance/CoFranceLoader.dll"))
            .unwrap()
            .trim(),
        "loader"
    );
    assert!(files.contains(&"LFXX/Plugins/CoFrance/CoFrance.ini".to_string()));

    // Pre-existing user state untouched.
    let user_state =
        fs::read_to_string(install_root.join("LFXX/Plugins/CoFrance/user_settings.dat")).unwrap();
    assert_eq!(user_state.trim(), "user state");

    // .prf comes from GitHub even though the package ships one — and it lands at
    // the FIR folder root, not a Profiles/ subfolder.
    let prf = fs::read_to_string(install_root.join("LFFF/EGA Paris.prf")).unwrap();
    assert_eq!(prf.trim(), "github prf");
    assert!(!files.iter().any(|f| f.contains("/Profiles/")), "got: {files:#?}");

    // Copyright comes from GitHub, not the package.
    let copyright = fs::read_to_string(install_root.join("LFBB/aeronav_copyright.txt")).unwrap();
    assert_eq!(copyright.trim(), "(c) AeroNav");
    let marker = fs::read_to_string(install_root.join("LFXX/Sectors/current_airac.txt")).unwrap();
    assert_eq!(marker.trim(), "2603");
    assert!(files.contains(&".github/installer-version.txt".to_string()));

    assert!(summary.files_written > 0);
    assert_eq!(summary.airac_cycle.as_deref(), Some("2603"));
    assert_eq!(summary.github_sha.as_deref(), Some("abcdef1"));
}

#[test]
fn second_run_is_a_no_op() {
    let (_gh_tmp, github_root) = build_fake_github_repo();
    let (_gng_tmp, gng_root) = build_fake_gng_package();
    let install_tmp = build_existing_install();
    let install_root = install_tmp.path();
    // Settings already match GitHub, so the run (incl. its settings backup)
    // stabilises after the first apply.
    write_file(install_root, "LFXX/Settings/Symbology.txt", "sym\n");
    let gng_roots = vec![gng_root.clone()];

    let inputs = || PlanInputs {
        github_root: Some(&github_root),
        gng_roots: &gng_roots,
        install_root,
        github_short_sha: Some("abcdef1".into()),
        area_source: AreaSource::Packages,
    };

    apply(install_root, &plan(inputs()).unwrap()).unwrap();
    let summary = apply(install_root, &plan(inputs()).unwrap()).unwrap();
    assert_eq!(summary.files_written, 0, "second run should be a no-op");
}

#[test]
fn preexisting_folders_without_a_package_are_kept() {
    let (_gh_tmp, github_root) = build_fake_github_repo();
    let (_gng_tmp, gng_root) = build_fake_gng_package(); // covers LFBB + LFFF only
    let install_tmp = TempDir::new().unwrap();
    let install_root = install_tmp.path();

    // Pre-existing folders for an uncovered FIR and the secret area: a previous
    // install the user is NOT re-selecting this run. They must survive untouched
    // — not re-picking a package must never destroy an existing install.
    write_file(install_root, "LFEE/ASR/old.asr", "stale\n");
    write_file(install_root, "LFFM/old.txt", "stale secret\n");

    let gng_roots = vec![gng_root.clone()];
    apply(
        install_root,
        &plan(PlanInputs {
            github_root: Some(&github_root),
            gng_roots: &gng_roots,
            install_root,
            github_short_sha: Some("abcdef1".into()),
        area_source: AreaSource::Packages,
        })
        .unwrap(),
    )
    .unwrap();

    let files = list_files(install_root);
    // The uncovered, previously-installed folders are preserved as-is...
    assert!(
        files.contains(&"LFEE/ASR/old.asr".to_string()),
        "LFEE wrongly removed: {files:#?}"
    );
    assert!(
        files.contains(&"LFFM/old.txt".to_string()),
        "LFFM wrongly removed: {files:#?}"
    );
    // ...and the selected package's FIR is installed alongside them.
    assert!(files.contains(&"LFBB/ASR/Tower.asr".to_string()));
}

#[test]
fn lffm_is_installed_when_its_package_is_present() {
    let (_gh_tmp, github_root) = build_fake_github_repo();
    let install_tmp = TempDir::new().unwrap();
    let install_root = install_tmp.path();

    // A package that covers the secret LFFM area.
    let pkg_tmp = TempDir::new().unwrap();
    let pkg = pkg_tmp.path().to_path_buf();
    write_file(&pkg, "LFFM/ICAO/airports.txt", "lffm icao\n");

    let gng_roots = vec![pkg.clone()];
    apply(
        install_root,
        &plan(PlanInputs {
            github_root: Some(&github_root),
            gng_roots: &gng_roots,
            install_root,
            github_short_sha: Some("abcdef1".into()),
        area_source: AreaSource::Packages,
        })
        .unwrap(),
    )
    .unwrap();

    let files = list_files(install_root);
    assert!(
        files.contains(&"LFFM/Secret.txt".to_string()),
        "LFFM GitHub folder should be installed when its package is present; got: {files:#?}"
    );
}

#[test]
fn github_only_refresh_updates_installed_areas_without_packages() {
    let (_gh_tmp, github_root) = build_fake_github_repo();
    let install_tmp = TempDir::new().unwrap();
    let install_root = install_tmp.path();

    // Areas already installed (no packages provided this run).
    write_file(install_root, "LFBB/ASR/old.asr", "old\n");
    write_file(install_root, "LFFF/ASR/old.asr", "old\n");

    apply(
        install_root,
        &plan(PlanInputs {
            github_root: Some(&github_root),
            gng_roots: &[],
            install_root,
            github_short_sha: Some("abcdef1".into()),
            area_source: AreaSource::InstalledOnly,
        })
        .unwrap(),
    )
    .unwrap();

    let files = list_files(install_root);
    // GitHub files refreshed for the installed areas + the shared base...
    assert!(files.contains(&"LFBB/ASR/Tower.asr".to_string()));
    assert!(files.contains(&"LFFF/ASR/Paris.asr".to_string()));
    assert!(files.contains(&"LFXX/Settings/Symbology.txt".to_string()));
    // ...and the installed folders are kept, not removed.
    assert!(files.contains(&"LFBB/ASR/old.asr".to_string()));
    // Areas that were NOT installed stay absent (LFEE has no folder; LFFM secret).
    assert!(!files.iter().any(|f| f.starts_with("LFEE/")), "got: {files:#?}");
    assert!(!files.iter().any(|f| f.starts_with("LFFM/")), "got: {files:#?}");
}

#[test]
fn lfxxn_package_fans_out_to_lfff_and_lfee() {
    let (_gh_tmp, github_root) = build_fake_github_repo();
    let (_pkg_tmp, pkg_root) = build_fake_lfxxn_package();
    let install_tmp = TempDir::new().unwrap();
    let install_root = install_tmp.path();

    let gng_roots = vec![pkg_root.clone()];
    let plan = plan(PlanInputs {
        github_root: Some(&github_root),
        gng_roots: &gng_roots,
        install_root,
        github_short_sha: Some("abcdef1".into()),
        area_source: AreaSource::Packages,
    })
    .unwrap();
    let summary = apply(install_root, &plan).unwrap();
    let files = list_files(install_root);

    // The single combined source is written as BOTH LFFF and LFEE sectors.
    for (fir, ext, content) in [
        ("LFFF", "sct", "north sct"),
        ("LFEE", "sct", "north sct"),
        ("LFFF", "ese", "north ese"),
        ("LFEE", "ese", "north ese"),
        ("LFFF", "rwy", "north rwy"),
        ("LFEE", "rwy", "north rwy"),
    ] {
        let body =
            fs::read_to_string(install_root.join(format!("LFXX/Sectors/{fir}.{ext}"))).unwrap();
        assert_eq!(body.trim(), content, "LFXX/Sectors/{fir}.{ext}");
    }

    // AIRAC parsed past the `_`-prefixed timestamp → 2605.
    let marker = fs::read_to_string(install_root.join("LFXX/Sectors/current_airac.txt")).unwrap();
    assert_eq!(marker.trim(), "2605");
    assert_eq!(summary.airac_cycle.as_deref(), Some("2605"));

    // GitHub overlay folders for BOTH covered FIRs are installed...
    assert!(files.contains(&"LFFF/ASR/Paris.asr".to_string()));
    assert!(files.contains(&"LFEE/ASR/Reims.asr".to_string()));
    // ...and no unrelated FIR overlay is (LFBB has no package here; LFFM secret).
    assert!(!files.iter().any(|f| f.starts_with("LFBB/")), "got: {files:#?}");
    assert!(!files.iter().any(|f| f.starts_with("LFFM/")), "got: {files:#?}");
}

#[test]
fn lfxxn_backs_up_existing_lfff_and_lfee_sectors() {
    let (_gh_tmp, github_root) = build_fake_github_repo();
    let (_pkg_tmp, pkg_root) = build_fake_lfxxn_package();
    let install_tmp = TempDir::new().unwrap();
    let install_root = install_tmp.path();

    // Pre-existing LFFF/LFEE sectors at the previous AIRAC cycle.
    write_file(install_root, "LFXX/Sectors/LFFF.sct", "old paris\n");
    write_file(install_root, "LFXX/Sectors/LFEE.sct", "old reims\n");
    write_file(install_root, "LFXX/Sectors/current_airac.txt", "2602\n");

    let gng_roots = vec![pkg_root.clone()];
    apply(
        install_root,
        &plan(PlanInputs {
            github_root: Some(&github_root),
            gng_roots: &gng_roots,
            install_root,
            github_short_sha: Some("abcdef1".into()),
            area_source: AreaSource::Packages,
        })
        .unwrap(),
    )
    .unwrap();

    // Both prior sectors were backed up under the previous AIRAC before overwrite.
    let lfff_bak =
        fs::read_to_string(install_root.join("LFXX/Sectors/Backup/LFFF-2602.sct")).unwrap();
    assert_eq!(lfff_bak.trim(), "old paris");
    let lfee_bak =
        fs::read_to_string(install_root.join("LFXX/Sectors/Backup/LFEE-2602.sct")).unwrap();
    assert_eq!(lfee_bak.trim(), "old reims");
    // ...and the new combined content is in place.
    let lfff = fs::read_to_string(install_root.join("LFXX/Sectors/LFFF.sct")).unwrap();
    assert_eq!(lfff.trim(), "north sct");
}

#[test]
fn lfxxn_second_run_is_a_no_op() {
    let (_gh_tmp, github_root) = build_fake_github_repo();
    let (_pkg_tmp, pkg_root) = build_fake_lfxxn_package();
    let install_tmp = TempDir::new().unwrap();
    let install_root = install_tmp.path();
    // Settings already match GitHub so the settings-backup step stabilises after
    // the first apply (mirrors `second_run_is_a_no_op`).
    write_file(install_root, "LFXX/Settings/Symbology.txt", "sym\n");
    let gng_roots = vec![pkg_root.clone()];

    let inputs = || PlanInputs {
        github_root: Some(&github_root),
        gng_roots: &gng_roots,
        install_root,
        github_short_sha: Some("abcdef1".into()),
        area_source: AreaSource::Packages,
    };

    apply(install_root, &plan(inputs()).unwrap()).unwrap();
    let summary = apply(install_root, &plan(inputs()).unwrap()).unwrap();
    assert_eq!(summary.files_written, 0, "second LFXXN run should be a no-op");
    // No spurious backups were created for the unchanged combined targets.
    let files = list_files(install_root);
    assert!(
        !files.iter().any(|f| f.starts_with("LFXX/Sectors/Backup/")),
        "no backups expected on a no-op run; got: {files:#?}"
    );
}
