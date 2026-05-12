from pathlib import Path
import re

ROOT_DIR = Path.cwd()
DRY_RUN = False


# =========================================================
# Helpers
# =========================================================

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
# 1. ASR cleanup
# =========================================================

def cleanup_sector_lines(asr_file: Path) -> None:
    lines = read_lines(asr_file)

    changed = False
    output = []

    for line in lines:
        if line.startswith("SECTORFILE:"):
            output.append("SECTORFILE:")
            if line != "SECTORFILE:":
                changed = True

        elif line.startswith("SECTORTITLE:"):
            output.append("SECTORTITLE:")
            if line != "SECTORTITLE:":
                changed = True

        else:
            output.append(line)

    if changed:
        print(f"[ASR] Cleaned sector lines: {asr_file}")
        write_lines(asr_file, output)


# =========================================================
# 2. AVISO filtering
# =========================================================

def get_icao_from_filename(path: Path) -> str | None:
    match = re.match(r"^([A-Z]{4})\\b", path.stem.upper())
    return match.group(1) if match else None


def get_tree_icao(line: str) -> str | None:
    match = re.match(
        r"^\\s*(Free Text|Geo|Regions)\\s*:\\s*([A-Z]{4})\\b",
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
# 3. PRF sync
# =========================================================

def sync_prf(prf_file: Path) -> None:
    lines = read_lines(prf_file)

    fastkeys = {}
    recent_indexes = {}

    for index, line in enumerate(lines):
        parts = line.split("\\t")

        if len(parts) < 3:
            continue

        section = parts[0].strip()
        key = parts[1].strip()
        value = normalize_asr_path(parts[2].strip())

        if section == "ASRFastKeys" and key.isdigit():
            number = int(key)

            if 1 <= number <= 9:
                fastkeys[number] = value

        elif section == "RecentFiles":
            match = re.match(r"^Recent(\\d+)$", key, re.IGNORECASE)

            if match:
                number = int(match.group(1))

                if 1 <= number <= 9:
                    recent_indexes[number] = index

    changed = False

    for number, expected_path in fastkeys.items():
        if number not in recent_indexes:
            continue

        line_index = recent_indexes[number]

        new_line = f"RecentFiles\\tRecent{number}\\t{expected_path}"

        if lines[line_index] != new_line:
            print(f"[PRF] Updating {prf_file} Recent{number}")
            lines[line_index] = new_line
            changed = True

    if changed:
        write_lines(prf_file, lines)


# =========================================================
# Main
# =========================================================

def find_package_roots():
    package_roots = []

    for asr_dir in ROOT_DIR.rglob("ASR"):
        if asr_dir.is_dir():
            package_roots.append(asr_dir.parent)

    return sorted(set(package_roots))


def main():
    package_roots = find_package_roots()

    for package_root in package_roots:
        print(f"===== Processing {package_root} =====")

        asr_dir = package_root / "ASR"
        settings_dir = package_root / "Settings"

        if asr_dir.exists():
            for asr_file in asr_dir.rglob("*.asr"):
                cleanup_sector_lines(asr_file)
                filter_aviso(asr_file)

        if settings_dir.exists():
            for prf_file in settings_dir.rglob("*.prf"):
                sync_prf(prf_file)

    print("Done.")


if __name__ == "__main__":
    main()
