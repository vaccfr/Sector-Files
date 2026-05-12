from pathlib import Path
import re
import shutil

ROOT_DIR = Path.cwd()
CREATE_BACKUPS = False
DRY_RUN = False

TARGET_PREFIXES = ("Free Text", "Geo", "Regions")


# =========================================================
# ASR CLEANUP
# =========================================================

def cleanup_asr_file(asr_file: Path):
    try:
        content = asr_file.read_text(encoding="utf-8", errors="ignore")
        lines = content.splitlines()

        updated_lines = []
        changed = False

        for line in lines:
            if line.startswith("SECTORFILE:"):
                updated_lines.append("SECTORFILE:")
                changed = True
            elif line.startswith("SECTORTITLE:"):
                updated_lines.append("SECTORTITLE:")
                changed = True
            else:
                updated_lines.append(line)

        if changed and not DRY_RUN:
            asr_file.write_text("\n".join(updated_lines) + "\n", encoding="utf-8")
            print(f"[ASR CLEANUP] Updated: {asr_file}")

    except Exception as e:
        print(f"[ASR CLEANUP] Failed {asr_file}: {e}")


# =========================================================
# AVISO FILTER
# =========================================================

def get_icao_from_filename(path: Path) -> str | None:
    match = re.search(r"\b(LF[A-Z0-9]{2})\b", path.stem.upper())
    return match.group(1) if match else None


def get_tree_icao(line: str) -> str | None:
    match = re.match(
        r"^\s*(Free Text|Geo|Regions)\s*:\s*([A-Z]{4})\b",
        line,
        re.IGNORECASE,
    )
    return match.group(2).upper() if match else None


def filter_aviso(asr_file: Path):
    if "AVISO" not in asr_file.stem.upper():
        return

    file_icao = get_icao_from_filename(asr_file)

    if not file_icao:
        print(f"[AVISO] SKIPPED no ICAO: {asr_file.name}")
        return

    try:
        original = asr_file.read_text(encoding="utf-8", errors="ignore")
        lines = original.splitlines()

        new_lines = []
        removed = 0

        for line in lines:
            tree_icao = get_tree_icao(line)

            if tree_icao:
                if tree_icao == file_icao:
                    new_lines.append(line)
                else:
                    removed += 1
            else:
                new_lines.append(line)

        if removed:
            print(f"[AVISO] Updated {asr_file.name} removed={removed}")

            if not DRY_RUN:
                if CREATE_BACKUPS:
                    shutil.copy2(asr_file, asr_file.with_suffix(asr_file.suffix + ".bak"))

                asr_file.write_text(
                    "\n".join(new_lines) + "\n",
                    encoding="utf-8",
                )

    except Exception as e:
        print(f"[AVISO] FAILED {asr_file}: {e}")


# =========================================================
# PRF RECENTFILES CHECK
# =========================================================

FASTKEY_REGEX = re.compile(
    r"^(ASRFastKeys\s*\d+)\s*=\s*(.+)$",
    re.IGNORECASE,
)

RECENT_REGEX = re.compile(
    r"^(Recent(?:Files)?\s*Recent?\s*(\d+)|Recent\s*(\d+))\s*=\s*(.+)$",
    re.IGNORECASE,
)


def normalize_asr_path(path_str: str) -> str:
    path_str = path_str.replace("/", "\\")

    idx = path_str.upper().find("\\ASR\\")
    if idx >= 0:
        path_str = path_str[idx:]

    return path_str.strip()



def update_prf(prf_file: Path):
    try:
        lines = prf_file.read_text(encoding="utf-8", errors="ignore").splitlines()

        fastkeys = {}
        recent_indexes = {}

        for i, line in enumerate(lines):
            fk_match = FASTKEY_REGEX.match(line)
            if fk_match:
                key_name = fk_match.group(1)
                number_match = re.search(r"(\d+)", key_name)

                if number_match:
                    idx = number_match.group(1)
                    fastkeys[idx] = normalize_asr_path(fk_match.group(2))

            recent_match = RECENT_REGEX.match(line)
            if recent_match:
                idx = recent_match.group(2) or recent_match.group(3)
                recent_indexes[idx] = i

        changed = False

        for idx, asr_path in fastkeys.items():
            if idx in recent_indexes:
                line_index = recent_indexes[idx]
                current_line = lines[line_index]

                left_side = current_line.split("=", 1)[0]
                new_line = f"{left_side}={asr_path}"

                if current_line != new_line:
                    print(f"[PRF] Updating {prf_file.name} Recent{idx}")
                    lines[line_index] = new_line
                    changed = True

        if changed and not DRY_RUN:
            if CREATE_BACKUPS:
                shutil.copy2(prf_file, prf_file.with_suffix(prf_file.suffix + ".bak"))

            prf_file.write_text("\n".join(lines) + "\n", encoding="utf-8")

    except Exception as e:
        print(f"[PRF] FAILED {prf_file}: {e}")


# =========================================================
# MAIN
# =========================================================

for fir_dir in ROOT_DIR.iterdir():
    if not fir_dir.is_dir():
        continue

    print(f"\n===== Processing FIR: {fir_dir.name} =====")

    for asr_file in fir_dir.rglob("*.asr"):
        cleanup_asr_file(asr_file)
        filter_aviso(asr_file)

    for prf_file in fir_dir.rglob("*.prf"):
        update_prf(prf_file)

print("\nDone.")