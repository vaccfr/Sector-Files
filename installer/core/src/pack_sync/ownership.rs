use globset::{Glob, GlobSet, GlobSetBuilder};
use std::path::Path;

/// Paths provided by the FIR packages rather than the GitHub repo, so the
/// GitHub overlay MUST NOT write to any path matching this list. The packages
/// contribute sector files (`.sct`/`.ese`, renamed into `LFXX/Sectors`), the
/// per-FIR `ICAO`/`NavData` folders (also mirrored into `LFXX`), and the
/// per-FIR `Settings/VoiceChannels.txt` (the only Settings file that is
/// GNG-sourced; the `.prf` `SettingsfileVOICE` line points at the FIR-local
/// `\Settings\VoiceChannels.txt`). Everything else — `.prf`, the rest of
/// Settings, plugins (incl. CoFrance), Alias, etc. — comes from GitHub.
///
/// Patterns are matched against the *destination* path inside the install
/// root, with forward slashes (e.g. "LFXX/Sectors/LFBB.sct").
pub const GNG_OWNED_PATHS: &[&str] = &[
    // Sector files (the package overlay renames them into LFXX/Sectors).
    "LFXX/Sectors",
    "LFXX/Sectors/**",
    // ICAO / NavData, both the LFXX mirror and the per-FIR sources.
    "LFXX/ICAO",
    "LFXX/ICAO/**",
    "LFXX/NavData",
    "LFXX/NavData/**",
    "LFBB/ICAO",
    "LFBB/ICAO/**",
    "LFBB/NavData",
    "LFBB/NavData/**",
    "LFEE/ICAO",
    "LFEE/ICAO/**",
    "LFEE/NavData",
    "LFEE/NavData/**",
    "LFFF/ICAO",
    "LFFF/ICAO/**",
    "LFFF/NavData",
    "LFFF/NavData/**",
    "LFFM/ICAO",
    "LFFM/ICAO/**",
    "LFFM/NavData",
    "LFFM/NavData/**",
    "LFMM/ICAO",
    "LFMM/ICAO/**",
    "LFMM/NavData",
    "LFMM/NavData/**",
    "LFRR/ICAO",
    "LFRR/ICAO/**",
    "LFRR/NavData",
    "LFRR/NavData/**",
    // Per-FIR voice-channel definitions. The only Settings file shipped by the
    // packages; the rest of each FIR's Settings folder is GitHub-provided.
    "LFBB/Settings/VoiceChannels.txt",
    "LFEE/Settings/VoiceChannels.txt",
    "LFFF/Settings/VoiceChannels.txt",
    "LFFM/Settings/VoiceChannels.txt",
    "LFMM/Settings/VoiceChannels.txt",
    "LFRR/Settings/VoiceChannels.txt",
];

pub fn gng_owned_set() -> GlobSet {
    let mut builder = GlobSetBuilder::new();
    for pattern in GNG_OWNED_PATHS {
        builder.add(Glob::new(pattern).expect("invalid built-in pattern"));
    }
    builder.build().expect("invalid built-in glob set")
}

pub fn rel_path_str(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub fn is_gng_owned(set: &GlobSet, rel: &Path) -> bool {
    set.is_match(rel_path_str(rel))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn sectors_directory_and_contents_are_owned() {
        let set = gng_owned_set();
        assert!(is_gng_owned(&set, &p("LFXX/Sectors")));
        assert!(is_gng_owned(&set, &p("LFXX/Sectors/LFBB.sct")));
        assert!(is_gng_owned(&set, &p("LFXX/Sectors/Backup/LFBB-2605.sct")));
    }

    #[test]
    fn per_fir_navdata_and_icao_are_owned() {
        let set = gng_owned_set();
        assert!(is_gng_owned(&set, &p("LFBB/ICAO/something.txt")));
        assert!(is_gng_owned(&set, &p("LFRR/NavData/sub/deep.dat")));
        assert!(is_gng_owned(&set, &p("LFXX/NavData/airways.txt")));
    }

    #[test]
    fn per_fir_voice_channels_are_owned() {
        let set = gng_owned_set();
        assert!(is_gng_owned(&set, &p("LFBB/Settings/VoiceChannels.txt")));
        assert!(is_gng_owned(&set, &p("LFRR/Settings/VoiceChannels.txt")));
        assert!(is_gng_owned(&set, &p("LFFF\\Settings\\VoiceChannels.txt")));
    }

    #[test]
    fn github_provided_paths_are_not_gng_owned() {
        let set = gng_owned_set();
        assert!(!is_gng_owned(&set, &p("LFBB/ASR/something.asr")));
        assert!(!is_gng_owned(&set, &p("LFXX/Settings/Symbology.txt")));
        assert!(!is_gng_owned(&set, &p("LFXX/Plugins/CCAMS/CCAMS.dll")));
        // Now GitHub-provided: profiles, CoFrance, Alias, per-FIR settings.
        assert!(!is_gng_owned(&set, &p("LFBB/EGA Paris.prf")));
        assert!(!is_gng_owned(&set, &p("LFXX/Plugins/CoFrance/CoFranceLoader.dll")));
        assert!(!is_gng_owned(&set, &p("LFXX/Alias/Alias.txt")));
        // Only VoiceChannels.txt is GNG-sourced; the rest of Settings is GitHub's.
        assert!(!is_gng_owned(&set, &p("LFFF/Settings/LoginProfiles.txt")));
        assert!(!is_gng_owned(&set, &p("LFBB/Settings/LoginProfiles.txt")));
    }

    #[test]
    fn paths_with_backslashes_normalize() {
        let set = gng_owned_set();
        assert!(is_gng_owned(&set, &p("LFXX\\Sectors\\LFBB.sct")));
    }
}
