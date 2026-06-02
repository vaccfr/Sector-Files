use crate::fir::FirCode;
use regex::Regex;
use std::sync::OnceLock;

/// Parse a GNG-style sector/profile filename and extract the FIR code and
/// AIRAC cycle if present.
///
/// Examples:
///   "LFBB-Bordeaux-260301-0003.sct"  → (LFBB, "2603")
///   "LFEE-Reims-260301-0003.ese"     → (LFEE, "2603")
///   "LFFM-Base-260301-0003.sct"      → None (LFFM is rejected — legacy base)
///   "garbage.sct"                    → None
///
/// AIRAC encoding: the 6-digit middle group is taken as `YYMMSS` where the
/// first 4 chars (YYMM) are the cycle identifier. If the GNG convention turns
/// out to use the full 6 digits instead, change `cycle_from_six_digits`.
pub fn parse_gng_sector_filename(name: &str) -> Option<(FirCode, Option<String>)> {
    // Look for a leading FIR code followed by a delimiter we recognise.
    let fir = leading_fir_code(name)?;

    // FIR found. Try to extract an AIRAC cycle. The cycle is the first
    // 6-digit numeric group found between dashes.
    let cycle = SIX_DIGIT_GROUP
        .get_or_init(|| Regex::new(r"-(\d{6})-").expect("regex"))
        .captures(name)
        .and_then(|c| c.get(1))
        .map(|m| cycle_from_six_digits(m.as_str()));

    Some((fir, cycle))
}

static SIX_DIGIT_GROUP: OnceLock<Regex> = OnceLock::new();

fn leading_fir_code(name: &str) -> Option<FirCode> {
    let upper = name.to_ascii_uppercase();
    // Reject the legacy LFFM base-pack prefix outright.
    if upper.starts_with("LFFM") {
        return None;
    }
    // Match exactly one of the known FIR codes at the start, followed by
    // either a separator (`-`, `_`, ` `) or a dot (for `LFBB.sct`-style).
    for fir in FirCode::ALL {
        let prefix = fir.as_str();
        if upper.starts_with(prefix) {
            let rest = &upper[prefix.len()..];
            if rest.is_empty() || matches!(rest.chars().next(), Some('-' | '_' | ' ' | '.')) {
                return Some(fir);
            }
        }
    }
    None
}

fn cycle_from_six_digits(six: &str) -> String {
    // Take the first 4 digits as the AIRAC cycle code (YYMM).
    // The last two digits typically encode an AIRAC sub-revision.
    six[..4].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_standard_gng_sector_filename() {
        let (fir, cycle) = parse_gng_sector_filename("LFBB-Bordeaux-260301-0003.sct").unwrap();
        assert_eq!(fir, FirCode::LFBB);
        assert_eq!(cycle.as_deref(), Some("2603"));
    }

    #[test]
    fn parses_ese_extension() {
        let (fir, _) = parse_gng_sector_filename("LFEE-Reims-260301-0003.ese").unwrap();
        assert_eq!(fir, FirCode::LFEE);
    }

    #[test]
    fn rejects_lffm_legacy_base() {
        assert!(parse_gng_sector_filename("LFFM-Base-260301-0003.sct").is_none());
    }

    #[test]
    fn rejects_unrelated_filenames() {
        assert!(parse_gng_sector_filename("README.txt").is_none());
        assert!(parse_gng_sector_filename("garbage.sct").is_none());
        assert!(parse_gng_sector_filename("LFXX-Base.sct").is_none());
    }

    #[test]
    fn parses_bare_fir_filename_without_cycle() {
        let (fir, cycle) = parse_gng_sector_filename("LFBB.sct").unwrap();
        assert_eq!(fir, FirCode::LFBB);
        assert!(cycle.is_none());
    }

    #[test]
    fn case_insensitive_fir_match() {
        let (fir, _) = parse_gng_sector_filename("lfmm-Marseille-260301-0003.sct").unwrap();
        assert_eq!(fir, FirCode::LFMM);
    }

    #[test]
    fn does_not_match_lfxx_or_other_lf_codes_as_fir() {
        // LFXX is the shared pack code, not a FIR — should not parse as one.
        assert!(parse_gng_sector_filename("LFXX-Base-260301-0003.sct").is_none());
        // LFXY is gibberish — not a known FIR.
        assert!(parse_gng_sector_filename("LFXY-Random.sct").is_none());
    }
}
