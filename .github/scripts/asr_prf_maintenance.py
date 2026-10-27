from pathlib import Path
import re

ROOT_DIR = Path.cwd()
DRY_RUN = False

FIR_FOLDER_RE = re.compile(r"^LF[A-Z0-9]{2}$", re.IGNORECASE)


def read_lines(path: Path) -> list[str]:
    return path.read_text(encoding="utf-8", errors="ignore").splitlines()


def write_lines(path: Path, lines: list[str]) -> None:
    if not DRY_RUN:
        path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def normalize_asr_path(value: str) -> str:
    value = value.strip().replace("/", "\\")
    idx = value.upper().find("\\ASR\\")
    if idx >= 0:
        value = value[idx:]
    return value


# =========================================================
# ASR cleanup
# =========================================================

def cleanup_sector_lines(asr_file: Path) -> None:
    lines = read_lines(asr_file)
    changed = False
    output = []

    for line in lines:
        if line.startswith("SECTORFILE:"):
            output.append("SECTORFILE:")
            changed = changed or line != "SECTORFILE:"
        elif line.startswith("SECTORTITLE:"):
            output.append("SECTORTITLE:")
            changed = changed or line != "SECTORTITLE:"
        else:
            output.append(line)

    if changed:
        print(f"[ASR] Cleaned sector lines: {asr_file}")
        write_lines(asr_file, output)


# =========================================================
# AVISO cleanup
# =========================================================

def get_icao_from_filename(path: Path) -> str | None:
    match = re.match(r"^([A-Z]{4})\b", path.stem.upper())
    return match.group(1) if match else None


def get_tree_icao(line: str) -> str | None:
    match = re.match(
        r"^\s*(Free Text|Geo|Regions)\s*:\s*([A-Z]{4})\b",
        line,
        re.IGNORECASE,
    )
    return match.group(2).upper() if match else None


def filter_aviso(asr_file: Path) -> None:
    if asr_file.parent.name.upper() != "AVISOS":
        return
    if "AVISO" not in asr_file.stem.upper():
        return

    target_icao = get_icao_from_filename(asr_file)
    if not target_icao:
        return

    lines = read_lines(asr_file)
    output = []
    removed = 0

    for line in lines:
        tree_icao = get_tree_icao(line)
        if tree_icao and tree_icao != target_icao:
            removed += 1
            continue
        output.append(line)

    if removed:
        print(f"[AVISO] {asr_file}: removed {removed} invalid entries")
        write_lines(asr_file, output)


# =========================================================
# PRF sync
#
# Handles tab-separated format:
#   ASRFastKeys    2         \ASR\CCA Iroise\CCA Iroise - VFR.asr
#   RecentFiles    Recent2   \ASR\AVISOs\LFBO Lannion AVISO.asr
#
# Result:
#   RecentFiles    Recent2   \ASR\CCA Iroise\CCA Iroise - VFR.asr
# =========================================================

def sync_prf(prf_file: Path) -> None:
    lines = read_lines(prf_file)

    fastkeys: dict[int, str] = {}
    recent_indexes: dict[int, int] = {}

    for index, line in enumerate(lines):
        parts = line.split("\t")

        if len(parts) < 3:
            continue

        section = parts[0].strip()
        key = parts[1].strip()
        value = normalize_asr_path(parts[2])

        if section == "ASRFastKeys" and key.isdigit():
            number = int(key)
            if 1 <= number <= 9:
                fastkeys[number] = value

        elif section == "RecentFiles":
            match = re.fullmatch(r"Recent(\d+)", key, re.IGNORECASE)
            if match:
                number = int(match.group(1))
                if 1 <= number <= 9:
                    recent_indexes[number] = index

    if not fastkeys:
        return

    changed = False

    for number in range(1, 10):
        if number not in fastkeys:
            continue

        expected_path = fastkeys[number]
        expected_line = f"RecentFiles\tRecent{number}\t{expected_path}"

        if number in recent_indexes:
            line_index = recent_indexes[number]
            if lines[line_index] != expected_line:
                print(f"[PRF] {prf_file}: syncing Recent{number}")
                print(f"      old: {lines[line_index]}")
                print(f"      new: {expected_line}")
                lines[line_index] = expected_line
                changed = True
        else:
            print(f"[PRF] {prf_file}: adding missing Recent{number}")
            lines.append(expected_line)
            changed = True

    if changed:
        write_lines(prf_file, lines)


# =========================================================
# Discovery
# =========================================================

def find_asr_dirs() -> list[Path]:
    return sorted({p for p in ROOT_DIR.rglob("ASR") if p.is_dir()})


def find_prf_files() -> list[Path]:
    """
    Finds .prf files anywhere under FIR folders like:
      LFRR/CCA Iroise.prf
      LFEE/CCA Bale.prf
      LFBB/LFBB/Settings/example.prf

    This intentionally does NOT require the .prf to be inside Settings.
    """
    prf_files = []

    for top_level in ROOT_DIR.iterdir():
        if not top_level.is_dir():
            continue

        if top_level.name.startswith("."):
            continue

        # FIR folders: LFBB, LFEE, LFFF, LFFM, LFMM, LFRR, etc.
        # Also allows base pack folders if they contain PRFs.
        if FIR_FOLDER_RE.match(top_level.name) or top_level.name.startswith("LFXX"):
            prf_files.extend(top_level.rglob("*.prf"))

    return sorted(set(prf_files))


def main() -> None:
    asr_count = 0
    prf_count = 0

    for asr_dir in find_asr_dirs():
        print(f"===== Processing ASR folder: {asr_dir} =====")
        for asr_file in asr_dir.rglob("*.asr"):
            asr_count += 1
            cleanup_sector_lines(asr_file)
            filter_aviso(asr_file)

    print("===== Processing PRF files =====")
    for prf_file in find_prf_files():
        prf_count += 1
        sync_prf(prf_file)

    print(f"Done. Checked {asr_count} ASR files and {prf_count} PRF files.")


if __name__ == "__main__":
    main()
