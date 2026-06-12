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
    Some((fir, parse_airac_cycle(name)))
}

/// The GNG "combined" sector code for the northern France pack. Its single
/// sector file serves both the Paris (LFFF) and Reims (LFEE) FIRs.
pub const LFXXN_CODE: &str = "LFXXN";

/// Parse a GNG sector/profile filename into the *set* of FIRs it covers plus the
/// AIRAC cycle. Unlike [`parse_gng_sector_filename`], a combined code such as
/// `LFXXN` resolves to several FIRs (e.g. `LFXXN-Paris-Reims_…` → LFFF + LFEE).
/// A regular `<FIR>-…` filename resolves to that single FIR.
///
/// Examples:
///   "LFXXN-Paris-Reims_20260605153747-260501-0001.sct" → ([LFFF, LFEE], "2605")
///   "LFBB-Bordeaux-260301-0003.sct"                    → ([LFBB], "2603")
pub fn parse_gng_sector_target(name: &str) -> Option<(Vec<FirCode>, Option<String>)> {
    // A combined code (e.g. LFXXN) takes precedence; no FIR code is a prefix of
    // it, so the order relative to the single-FIR check is not load-bearing.
    let firs = if let Some(combined) = leading_combined_code(name) {
        combined.to_vec()
    } else {
        vec![leading_fir_code(name)?]
    };
    Some((firs, parse_airac_cycle(name)))
}

/// Extract the AIRAC cycle: the first 6-digit numeric group found between
/// dashes, of which the first 4 digits (YYMM) are the cycle. Returns `None` when
/// no such group is present (e.g. a bare `LFBB.sct`). The `_`-prefixed 14-digit
/// creation timestamp in newer GNG names is not delimited by dashes, so it is
/// never mistaken for the cycle.
fn parse_airac_cycle(name: &str) -> Option<String> {
    SIX_DIGIT_GROUP
        .get_or_init(|| Regex::new(r"-(\d{6})-").expect("regex"))
        .captures(name)
        .and_then(|c| c.get(1))
        .map(|m| cycle_from_six_digits(m.as_str()))
}

/// If `name` starts with a known combined code followed by a separator, the FIRs
/// it covers. Mirrors [`leading_fir_code`]'s prefix+separator matching so that
/// e.g. `LFXXNX` does not match.
fn leading_combined_code(name: &str) -> Option<&'static [FirCode]> {
    const COMBINED: &[(&str, &[FirCode])] =
        &[(LFXXN_CODE, &[FirCode::LFFF, FirCode::LFEE])];
    let upper = name.to_ascii_uppercase();
    for (code, firs) in COMBINED {
        if let Some(rest) = upper.strip_prefix(code) {
            if rest.is_empty() || matches!(rest.chars().next(), Some('-' | '_' | ' ' | '.')) {
                return Some(firs);
            }
        }
    }
    None
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

    #[test]
    fn combined_lfxxn_resolves_to_lfff_and_lfee() {
        // The real GNG combined name embeds an `_`-prefixed 14-digit creation
        // timestamp before the AIRAC group; the cycle must be the `-260501-`
        // group (→ 2605), not any 6 digits of the timestamp.
        let (firs, cycle) =
            parse_gng_sector_target("LFXXN-Paris-Reims_20260605153747-260501-0001.sct").unwrap();
        assert_eq!(firs, vec![FirCode::LFFF, FirCode::LFEE]);
        assert_eq!(cycle.as_deref(), Some("2605"));
    }

    #[test]
    fn combined_lfxxn_ese_variant() {
        let (firs, cycle) =
            parse_gng_sector_target("LFXXN-Paris-Reims_20260605153747-260501-0001.ese").unwrap();
        assert_eq!(firs, vec![FirCode::LFFF, FirCode::LFEE]);
        assert_eq!(cycle.as_deref(), Some("2605"));
    }

    #[test]
    fn combined_lfxxn_is_not_a_single_fir() {
        // The legacy single-FIR parser must not claim LFXXN as a FIR.
        assert!(parse_gng_sector_filename("LFXXN-Paris-Reims_20260605153747-260501-0001.sct")
            .is_none());
    }

    #[test]
    fn sector_target_falls_back_to_single_fir() {
        let (firs, cycle) =
            parse_gng_sector_target("LFBB-Bordeaux-260301-0003.sct").unwrap();
        assert_eq!(firs, vec![FirCode::LFBB]);
        assert_eq!(cycle.as_deref(), Some("2603"));
    }

    #[test]
    fn combined_code_requires_separator() {
        // A longer code that merely starts with LFXXN must not match.
        assert!(parse_gng_sector_target("LFXXNX-Random.sct").is_none());
    }
}
