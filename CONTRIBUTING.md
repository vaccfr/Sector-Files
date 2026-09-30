# Contributing

Thank you for contributing to the French vACC Sector File Repository.

This repository contains the configuration and supporting files used to build
the French vACC Controller Packs. Contributions should be made to the relevant
FIR or shared resource and submitted through a pull request.

The complete AIRAC sector packages are downloaded from AeroNav GNG by the
Controller Pack Installer and are not stored in this repository.

## Repository Structure

FIR-specific resources are stored in top-level folders named after the FIR:

```text
LFBB/
LFEE/
LFFF/
LFFM/
LFMM/
LFRR/
```

A typical FIR folder contains:

```text
{FIR}/
├── ASR/
├── Settings/
└── *.prf
```

Shared Controller Pack resources are stored under:

```text
LFXX/
├── Alias/
├── ASR/
├── Plugins/
├── Settings/
├── Sounds/
└── *.ttf
```

Other important locations are:

```text
installer/   Controller Pack Installer source
scripts/     Repository maintenance scripts
.github/     Issue templates, workflows and repository automation
```

Place a change in the most specific applicable folder. Use `LFXX` only for a
resource shared by multiple FIRs or by the complete Controller Pack.

## Types of Contribution

Common contributions include:

- EuroScope display files (`.asr`);
- EuroScope profile files (`.prf`);
- FIR-specific and shared settings;
- aliases, sounds, fonts and plugin configuration;
- approved plugin updates;
- Controller Pack Installer changes;
- documentation and repository automation.

Keep pull requests focused. Unrelated FIR, plugin, installer and formatting
changes should normally be submitted separately.

## Creating or Updating Controller Pack Files

1. Identify the affected FIR or shared `LFXX` resource.
2. Update only the files required for the change.
3. Open the affected profile in EuroScope and verify that it loads correctly.
4. Check displays, settings, plugins and paths affected by the change.
5. Remove local, temporary and backup files before committing.
6. Submit the change through a pull request to `main`.

Preserve the existing folder structure, filenames, text encoding and line
format unless the change specifically requires a migration.

## ASR and PRF Maintenance

After a pull request is merged, the `ASR and PRF maintenance` workflow runs
`scripts/asr_prf_maintenance.py`. It automatically:

- clears values from `SECTORFILE:` and `SECTORTITLE:` lines in `.asr` files;
- removes entries for other aerodromes from an AVISO display tree;
- synchronises PRF `RecentFiles` entries 1–9 with `ASRFastKeys` entries 1–9.

You can run the same maintenance locally before submitting:

```bash
python scripts/asr_prf_maintenance.py
```

Review all resulting changes before committing them. Do not add values that
the maintenance script intentionally removes or maintain duplicate PRF recent
file entries by hand.

## AVISO Display Files

AVISO `.asr` files are stored in the relevant FIR's `ASR/AVISOs/` directory.
The filename must begin with the four-letter aerodrome ICAO code used by the
display tree entries.

An AVISO display should contain only `Free Text`, `Geo` and `Regions` tree
entries belonging to that aerodrome. The maintenance workflow removes entries
whose ICAO code does not match the filename.

Source AVISO geometry is maintained in the separate QGIS-AVISO project. When
an AVISO change originates there, keep the corresponding source change and the
Controller Pack display update traceable in the issue or pull request.

## Plugin Updates

Plugin binaries and their supporting configuration are stored under
`LFXX/Plugins/`. Plugin updates must come from the documented upstream project
and must be tested in EuroScope before submission.

When an approved plugin version is deployed:

1. update the required binary and configuration files;
2. test that the plugin loads and its existing configuration remains valid;
3. update the matching `current` version in
   `.github/controller_pack_plugins.json`;
4. identify the upstream repository and release in the pull request.

Do not include unrelated plugin binaries or development artifacts.

## Controller Pack Installer

Installer source is stored under `installer/`. It uses React, TypeScript,
Tauri and Rust.

For installer changes, run the relevant checks from `installer/`:

```bash
bun install
bun run build
cargo test -p controller-pack-core
```

Also check the Tauri crate:

```bash
cd installer/src-tauri
cargo check --all-targets
```

The pull-request workflow repeats the core tests, frontend build and platform
smoke checks. Maintainers should follow `installer/RELEASE.md` when preparing a
release. Never commit signing keys, credentials or generated build artifacts.

## Sources and Accuracy

Operational data must be based on reliable and current information. Prefer
official sources such as:

- the French AIP;
- AIP amendments and supplements;
- official aerodrome and en-route charts;
- NOTAMs;
- approved French vACC documentation.

Identify the source used for a significant operational change in the pull
request. If a source is ambiguous or conflicts with the current pack, raise an
issue or ask a maintainer before making a broad change.

## AIRAC Changes

For an AIRAC-dependent pull request, enter the effective date in the pull
request template using `YYYY-MM-DD`.

Maintainers apply the `airac-dependent` label after verifying the date. The
`AIRAC merge gate` status remains pending until 00:00 UTC on that date. Do not
remove the label or merge the change early unless the effective date or scope
has been reviewed and corrected by a maintainer.

Non-AIRAC changes should use `Not applicable` for the AIRAC effective date.

## Pull Requests

A pull request should clearly explain:

- which FIR, profile, aerodrome, shared resource or installer component is
  affected;
- what was changed and why;
- how the change was tested;
- the source or reference used for operational data;
- the applicable AIRAC effective date, where relevant;
- any expected effect on existing profiles or Controller Pack users.

Include screenshots when they help reviewers verify an ASR, AVISO, interface
or visual configuration change.

Before requesting review, confirm that:

- the affected files load correctly;
- existing profiles and settings still work;
- repository maintenance has been considered;
- no temporary, generated or unnecessary files are included;
- the appropriate FIR or technical team has been notified when required.

## Issues

If you identify an issue but cannot submit the correction yourself, use the
[Sector File issue template](https://github.com/vaccfr/Sector-Files/issues/new?template=sector-file-issue.md).

Include the affected FIR, profile or aerodrome, a clear description, supporting
source material and the applicable AIRAC date when relevant.

## Questions

If you are unsure about the correct folder, operational source, EuroScope file
format, plugin update or installer workflow, ask before submitting a large
change. Early discussion helps keep the Controller Packs consistent and avoids
unnecessary rework.
