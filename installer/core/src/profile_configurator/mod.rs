use crate::profile_types::Profile;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub const RATINGS: &[&str] = &[
    "OBS", "S1", "S2", "S3", "C1", "C2", "C3", "I1", "I2", "I3", "SUP", "ADM",
];

pub const RPC_PATH: &str = r"\..\LFXX\Plugins\EuroscopeRPC\EuroscopeRPC.dll";
pub const LOGIN_PROFILES_PLACEHOLDER: &str = "VOTRE_CID_ICI";

pub fn rating_to_euroscope_value(rating: &str) -> String {
    RATINGS
        .iter()
        .position(|r| r.eq_ignore_ascii_case(rating.trim()))
        .map(|idx| idx.to_string())
        .unwrap_or_else(|| "1".to_string())
}

pub fn is_euroscope_rpc_line(line: &str) -> bool {
    let trimmed = line.trim();
    if !trimmed.starts_with("Plugins\tPlugin") {
        return false;
    }
    let normalized = trimmed.replace('/', "\\").to_ascii_lowercase();
    normalized.contains("lfxx\\plugins\\euroscoperpc\\euroscoperpc.dll")
}

pub fn extract_plugin_lines_from_prf(path: &Path) -> Vec<String> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    content
        .lines()
        .filter(|line| line.trim_start().starts_with("Plugins\tPlugin"))
        .map(|line| {
            if line.ends_with('\n') {
                line.to_string()
            } else {
                format!("{}\n", line)
            }
        })
        .collect()
}

pub fn ensure_rpc_plugin(lines: &mut Vec<String>) {
    if lines.iter().any(|l| is_euroscope_rpc_line(l)) {
        return;
    }
    let next = lines
        .iter()
        .filter_map(|line| {
            let rest = line.strip_prefix("Plugins\tPlugin")?;
            let num_end = rest.find('\t')?;
            rest[..num_end].parse::<u32>().ok()
        })
        .max()
        .unwrap_or(0)
        + 1;

    if let Some(last) = lines.last_mut() {
        if !last.ends_with('\n') {
            last.push('\n');
        }
    }
    lines.push(format!("Plugins\tPlugin{}\t{}\n", next, RPC_PATH));
}

#[derive(Debug, Clone)]
pub struct PatchDetails {
    pub real_name: String,
    pub cid: String,
    pub password: String,
    pub rating: String,
    pub enable_rpc: bool,
    pub example_plugin_lines: Vec<String>,
}

pub fn patch_prf_file(path: &Path, details: &PatchDetails) -> anyhow::Result<bool> {
    let original = fs::read_to_string(path)?;
    let mut lines: Vec<String> = original
        .split_inclusive('\n')
        .map(str::to_string)
        .collect();

    let mut output: Vec<String> = Vec::with_capacity(lines.len() + 8);
    let mut seen = SeenLastSession::default();

    for line in lines.drain(..) {
        let stripped = line.trim_end_matches(|c| c == '\n' || c == '\r').to_string();

        if stripped.starts_with("Plugins\tPlugin") && !details.example_plugin_lines.is_empty() {
            continue;
        }

        if is_euroscope_rpc_line(&stripped) && !details.enable_rpc {
            continue;
        }

        if let Some(new_line) = replace_last_session_line(&stripped, details, &mut seen) {
            output.push(new_line);
            continue;
        }

        output.push(line);
    }

    if let Some(last) = output.last_mut() {
        if !last.ends_with('\n') {
            last.push('\n');
        }
    }

    append_missing_last_session(&mut output, details, &seen);

    if !details.example_plugin_lines.is_empty() {
        if let Some(last) = output.last_mut() {
            if !last.ends_with('\n') {
                last.push('\n');
            }
        }
        output.extend(details.example_plugin_lines.iter().cloned());
    }

    if details.enable_rpc {
        ensure_rpc_plugin(&mut output);
    }

    let updated: String = output.concat();
    if updated == original {
        return Ok(false);
    }

    fs::write(path, updated)?;
    Ok(true)
}

#[derive(Default)]
struct SeenLastSession {
    realname: bool,
    certificate: bool,
    rating: bool,
    password: bool,
}

fn replace_last_session_line(
    stripped: &str,
    details: &PatchDetails,
    seen: &mut SeenLastSession,
) -> Option<String> {
    if stripped.starts_with("LastSession\trealname\t") {
        seen.realname = true;
        return Some(format!("LastSession\trealname\t{}\n", details.real_name));
    }
    if stripped.starts_with("LastSession\tcertificate\t") {
        seen.certificate = true;
        return Some(format!("LastSession\tcertificate\t{}\n", details.cid));
    }
    if stripped.starts_with("LastSession\trating\t") {
        seen.rating = true;
        return Some(format!(
            "LastSession\trating\t{}\n",
            rating_to_euroscope_value(&details.rating)
        ));
    }
    if stripped.starts_with("LastSession\tpassword\t") {
        seen.password = true;
        return Some(format!("LastSession\tpassword\t{}\n", details.password));
    }
    None
}

fn append_missing_last_session(
    output: &mut Vec<String>,
    details: &PatchDetails,
    seen: &SeenLastSession,
) {
    if !seen.realname {
        output.push(format!("LastSession\trealname\t{}\n", details.real_name));
    }
    if !seen.certificate {
        output.push(format!("LastSession\tcertificate\t{}\n", details.cid));
    }
    if !seen.rating {
        output.push(format!(
            "LastSession\trating\t{}\n",
            rating_to_euroscope_value(&details.rating)
        ));
    }
    if !seen.password {
        output.push(format!("LastSession\tpassword\t{}\n", details.password));
    }
}

pub fn patch_login_profiles_file(path: &Path, cid: &str) -> anyhow::Result<bool> {
    let original = fs::read_to_string(path)?;
    let updated = original.replace(LOGIN_PROFILES_PLACEHOLDER, cid);
    if updated == original {
        return Ok(false);
    }
    fs::write(path, updated)?;
    Ok(true)
}

pub fn find_prf_files(root: &Path) -> Vec<PathBuf> {
    WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_type().is_file()
                && e.path()
                    .extension()
                    .map(|ext| ext.eq_ignore_ascii_case("prf"))
                    .unwrap_or(false)
        })
        .map(|e| e.path().to_path_buf())
        .collect()
}

pub fn find_login_profiles_files(root: &Path) -> Vec<PathBuf> {
    WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file() && e.file_name() == "LoginProfiles.txt")
        .map(|e| e.path().to_path_buf())
        .collect()
}

pub fn apply(install_root: &Path, profile: &Profile) -> anyhow::Result<usize> {
    if profile.vatsim.cid.trim().is_empty() {
        tracing::info!("CID empty; profile configurator skipped");
        return Ok(0);
    }

    let details = PatchDetails {
        real_name: profile.vatsim.real_name.clone(),
        cid: profile.vatsim.cid.clone(),
        password: profile.vatsim.password.clone(),
        rating: profile.vatsim.rating.clone(),
        enable_rpc: profile.vatsim.enable_rpc,
        example_plugin_lines: vec![],
    };

    let mut changed = 0usize;
    for prf in find_prf_files(install_root) {
        if patch_prf_file(&prf, &details)? {
            changed += 1;
        }
    }
    for login_file in find_login_profiles_files(install_root) {
        if patch_login_profiles_file(&login_file, &profile.vatsim.cid)? {
            changed += 1;
        }
    }
    Ok(changed)
}

pub fn import_plugin_lines(install_root: &Path, example_prf: &Path) -> anyhow::Result<usize> {
    let plugin_lines = extract_plugin_lines_from_prf(example_prf);
    if plugin_lines.is_empty() {
        return Ok(0);
    }
    let mut changed = 0;
    for prf in find_prf_files(install_root) {
        if prf == example_prf {
            continue;
        }
        let original = fs::read_to_string(&prf)?;
        let mut kept: Vec<String> = original
            .split_inclusive('\n')
            .filter(|line| !line.trim_start().starts_with("Plugins\tPlugin"))
            .map(str::to_string)
            .collect();
        if let Some(last) = kept.last_mut() {
            if !last.ends_with('\n') {
                last.push('\n');
            }
        }
        kept.extend(plugin_lines.iter().cloned());
        let updated: String = kept.concat();
        if updated != original {
            fs::write(&prf, updated)?;
            changed += 1;
        }
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rating_to_value_known() {
        assert_eq!(rating_to_euroscope_value("OBS"), "0");
        assert_eq!(rating_to_euroscope_value("S1"), "1");
        assert_eq!(rating_to_euroscope_value("C1"), "4");
        assert_eq!(rating_to_euroscope_value("ADM"), "11");
    }

    #[test]
    fn rating_to_value_unknown_falls_back_to_1() {
        assert_eq!(rating_to_euroscope_value("Bogus"), "1");
    }

    #[test]
    fn detects_rpc_line() {
        assert!(is_euroscope_rpc_line(
            r"Plugins	Plugin5	\..\LFXX\Plugins\EuroscopeRPC\EuroscopeRPC.dll"
        ));
        assert!(is_euroscope_rpc_line(
            r"Plugins	Plugin1	../LFXX/Plugins/EuroscopeRPC/EuroscopeRPC.dll"
        ));
        assert!(!is_euroscope_rpc_line(r"Plugins	Plugin1	\..\LFXX\Plugins\Other.dll"));
        assert!(!is_euroscope_rpc_line("LastSession\trealname\tFoo"));
    }

    #[test]
    fn patch_prf_replaces_existing_and_appends_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("Test.prf");
        fs::write(
            &path,
            "LastSession\trealname\tOld\nLastSession\tcertificate\t999999\nOther\tline\tvalue\n",
        )
        .unwrap();

        let details = PatchDetails {
            real_name: "New Name".into(),
            cid: "1234567".into(),
            password: "secret".into(),
            rating: "C1".into(),
            enable_rpc: true,
            example_plugin_lines: vec![],
        };

        let changed = patch_prf_file(&path, &details).unwrap();
        assert!(changed);

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("LastSession\trealname\tNew Name\n"));
        assert!(content.contains("LastSession\tcertificate\t1234567\n"));
        assert!(content.contains("LastSession\trating\t4\n"));
        assert!(content.contains("LastSession\tpassword\tsecret\n"));
        assert!(content.contains("EuroscopeRPC.dll"));
    }

    #[test]
    fn patch_prf_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("Test.prf");
        fs::write(&path, "Other\tline\tvalue\n").unwrap();

        let details = PatchDetails {
            real_name: "X".into(),
            cid: "1234567".into(),
            password: "pw".into(),
            rating: "S2".into(),
            enable_rpc: false,
            example_plugin_lines: vec![],
        };

        assert!(patch_prf_file(&path, &details).unwrap());
        assert!(!patch_prf_file(&path, &details).unwrap());
    }

    #[test]
    fn login_profiles_placeholder_replaced() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("LoginProfiles.txt");
        fs::write(&path, "[Profile1]\nCID=VOTRE_CID_ICI\n").unwrap();
        assert!(patch_login_profiles_file(&path, "1234567").unwrap());
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("CID=1234567"));
        // Idempotent second call.
        assert!(!patch_login_profiles_file(&path, "1234567").unwrap());
    }

    #[test]
    fn rpc_disabled_removes_existing_line() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("Test.prf");
        fs::write(
            &path,
            "Plugins\tPlugin1\t\\..\\LFXX\\Plugins\\EuroscopeRPC\\EuroscopeRPC.dll\nOther\tline\tx\n",
        )
        .unwrap();
        let details = PatchDetails {
            real_name: "X".into(),
            cid: "1".into(),
            password: "p".into(),
            rating: "S1".into(),
            enable_rpc: false,
            example_plugin_lines: vec![],
        };
        patch_prf_file(&path, &details).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(!content.to_ascii_lowercase().contains("euroscoperpc"));
    }
}
