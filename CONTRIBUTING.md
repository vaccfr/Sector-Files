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
.github/scripts/  Repository maintenance scripts
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

## ASR and PRF Autofix

The `autofix.ci` workflow runs `.github/scripts/asr_prf_maintenance.py` on every
pull request to `main`. The autofix.ci GitHub App commits any fixes to the
PR branch, so they can be reviewed before merging. For contributions from
forks, enable **Allow edits by maintainers**.

The script automatically:

- clears values from `SECTORFILE:` and `SECTORTITLE:` lines in `.asr` files;
- removes entries for other aerodromes from an AVISO display tree;
- synchronises PRF `RecentFiles` entries 1-9 with `ASRFastKeys` entries 1-9.

There is no sector-data validation CLI. Contributors must still verify
operational changes in EuroScope. The installer retains its own build and
test workflow when installer files change.

You can optionally run the same maintenance locally:

```bash
python .github/scripts/asr_prf_maintenance.py
```

Review the resulting changes. Do not add values that the script intentionally
removes or maintain duplicate PRF recent file entries by hand.

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

For a change that must wait for an AIRAC effective date:

1. Tick the AIRAC dependency checkbox in the PR template.
2. Apply its `airac/XXXX` label, for example `airac/2611`. Ask
   `@vaccfr/navigation` to apply it if you do not have permission.
3. Obtain the relevant CODEOWNER reviews and let autofix and any installer
   checks finish. You can enable auto-merge while waiting.

**The checkbox alone does not activate the gate.** Cycle dates are defined in
[.github/airac-cycles.yml](.github/airac-cycles.yml). The `AIRAC merge gate`
commit status remains pending until 00:00 UTC on the effective date and opens
on the next scheduled evaluation (every 15 minutes, subject to GitHub delays).
It also runs on PR updates and can be run manually from the Actions page.

If several AIRAC labels are present, the latest effective date applies.
Unknown cycle labels fail the gate. Effective labels stay on open PRs; the
gate changes the status directly, so unlocking does not require another
workflow to be triggered by a bot's label removal. A bot comment explains the
current gate state.

The label-sync workflow maintains the next four upcoming cycle labels each
Monday, when the calendar changes, and on manual runs. It deletes out-of-window
labels only when no open PR uses them. Extend the calendar before it expires.

The daily `AIRAC tag` workflow creates an annotated `airac-XXXX` tag on
effective dates. This records the current `main` snapshot at the time of the
run; PRs merged afterwards are not included in that tag. It does not create an
installer release, trigger a deployment, or replace the separate merge gate.

Existing PRs with `airac-dependent` continue to use the effective date in
their description during migration. Apply the correct cycle label before
removing the old label. Missing or invalid legacy dates fail the gate.
Non-AIRAC changes need no AIRAC label.

Do not remove a dependency label to merge early unless Navigation has reviewed
and corrected the effective date or scope.

## Ownership and Reviews

[CODEOWNERS](.github/CODEOWNERS) assigns reviewers by folder:

| Area | Owner |
| --- | --- |
| Default, FIRs, shared LFXX resources, scripts, documentation and GitHub configuration | `@vaccfr/navigation` |
| `installer/` | `@vaccfr/navigation` |
| `LFXX/Plugins/CoFrance/`, `Central Squawk/`, `EuroscopeRPC/`, `Plugin Bridge/`, `RampAgent/` and `AMAN/` | `@vaccfr/digital` |

For an existing folder, obtain approval from its CODEOWNER before merging.
Authors cannot approve their own PRs. Changes spanning ownership areas need
review from the affected owners. Maintainers merge their PRs once reviews,
autofix, the AIRAC status and applicable installer checks pass.

## GitHub Setup for Repository Administrators

After merging this configuration into `main`:

1. Give the named owners write access to the repository. Teams used by
   CODEOWNERS must be visible and have explicit write access.
2. Install or enable the [autofix.ci GitHub App](https://autofix.ci/setup) for
   this repository. The workflow needs this service to apply fixes, including
   to fork PRs. No personal access token or custom AIRAC bot secrets are needed.
3. In the repository settings, enable **Allow auto-merge**.
4. Protect `main` with a branch rule or ruleset requiring pull requests,
   at least one approving review, **Require review from Code Owners**, and
   dismissal of stale approvals when new commits are pushed.
5. Require the **AIRAC merge gate** commit status and **ASR and PRF autofix**
   check after their first runs make them available. Select the AIRAC commit
   status, not the workflow job named `Evaluate AIRAC eligibility`: that job
   can succeed while the date status is pending.
6. Keep existing installer requirements for installer changes. Its checks are
   path-filtered, so do not make them unconditional requirements for all FIR
   PRs unless the installer workflow is adapted accordingly.
7. Run **Sync AIRAC labels** and **AIRAC merge gate** once from Actions to
   initialise labels and refresh existing PRs. Confirm a test PR requests the
   expected owners and stays blocked with a future cycle label.

These files define the automation; they do not change repository settings or
team permissions themselves. Scheduled gate/tag/label jobs run from the default
branch, which must remain `main`. Privileged AIRAC jobs execute only trusted
code from `main`; ASR/PRF autofix runs PR code with read-only credentials.

For changes to AIRAC automation, run its focused tests:

```bash
node --test .github/scripts/airac.test.cjs
```

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
