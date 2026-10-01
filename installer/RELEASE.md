# Releasing the Controller Pack Installer

This document is for **maintainers** who cut signed Tauri releases. End users do not need to read it.

## One-time setup

### 1. Generate a Tauri signing keypair

```bash
bun x @tauri-apps/cli signer generate -w ~/.tauri/cofrance-installer.key
```

You'll be prompted for a passphrase — **use one** and store it in your password manager. The command emits:

- `~/.tauri/cofrance-installer.key` — the **private** key. Never commit this. Never email it. Never upload it anywhere except the GitHub repo's Actions secrets (next step).
- `~/.tauri/cofrance-installer.key.pub` — the **public** key. Embed this in `installer/src-tauri/tauri.conf.json` under `plugins.updater.pubkey`.

### 2. Put the private key in GitHub Actions secrets

Repo → Settings → Secrets and variables → Actions → New repository secret:

- `TAURI_SIGNING_PRIVATE_KEY` — paste the **entire** content of `~/.tauri/cofrance-installer.key` (the file, not the path).
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — the passphrase you chose.

### 3. Commit the public key

Edit `installer/src-tauri/tauri.conf.json`:

```json
"plugins": {
  "updater": {
    "pubkey": "<paste the content of ~/.tauri/cofrance-installer.key.pub here>"
  }
}
```

Commit and push. Every subsequent build will be signed by the matching private key and verified against the public key embedded in the binary.

> ⚠️ **If you ever lose the private key, every existing installation is permanently orphaned** — they will refuse any update because they only trust manifests signed by that exact key. Back the key up in *at least* two places (e.g. password manager + offline encrypted USB).

The same keypair signs **all three** platforms' updater bundles — there is nothing extra to generate for macOS or Linux.

### 4. (Optional) Apple code signing & notarization

Entirely optional. Without it the macOS build still works and still auto-updates; it is just unsigned, so the **first** launch needs the Gatekeeper workaround (see [Installing the macOS build](#installing-the-macos-build)). With an Apple Developer Program membership ($99/yr), add these repository secrets and the workflow signs and notarizes automatically:

| Secret | What it is |
| --- | --- |
| `APPLE_CERTIFICATE` | base64 of the exported *Developer ID Application* `.p12` (`base64 -i cert.p12 \| pbcopy`) |
| `APPLE_CERTIFICATE_PASSWORD` | the password set when exporting the `.p12` |
| `APPLE_SIGNING_IDENTITY` | optional cross-check, e.g. `Developer ID Application: Your Org (TEAMID)` — the bundler signs with the identity inside the `.p12`, and fails loudly if this doesn't match it |
| `APPLE_ID` | Apple ID used for notarization |
| `APPLE_PASSWORD` | an **app-specific password** for that Apple ID |
| `APPLE_TEAM_ID` | the 10-character team ID |

Signing switches on when `APPLE_CERTIFICATE` is present; notarization additionally requires `APPLE_ID`. Leave the secrets unset and the job silently produces an unsigned build — it must never be set to an empty string, which the bundler reads as "sign with this" and fails on.

## Cutting a release

The pack ships as a **single combined release roughly once a month**. Any published GitHub Release — whether it carries AIRAC (sector) changes, installer changes, or both — triggers `.github/workflows/build-installer-tauri.yml`, which rebuilds, signs, and attaches the Windows x64 installer, the macOS universal app, the Linux x64 AppImage, and a `latest.json` covering all three to that same release. There is no dedicated installer-only release stream and no special tag prefix; the workflow keys off the release event, not the tag name.

### When the installer code changed

Bump the version *before* tagging so clients actually see the update — the updater compares the version in `latest.json` against the running app, so an unchanged version is treated as "no update available".

1. Bump the version in all three places (keep them in sync):
   - `installer/Cargo.toml` → `[workspace.package] version` (this drives both Rust crates)
   - `installer/src-tauri/tauri.conf.json` → top-level `version`
   - `installer/package.json` → `version`
2. Commit: `git commit -m "installer: bump to v0.2.0"`.

### When only the AIRAC / sector content changed

No version bump is needed. The release still rebuilds and re-signs the installer, but because the version is unchanged, existing installs see "no update" and keep running their current build. That's intended — the new sectors are picked up the next time a user runs the installer, not via the app updater.

### Publishing

1. Tag and create the GitHub Release as usual for the monthly drop (use whatever tag the release uses — the workflow does not require a prefix).
2. Three jobs run on `release: published`, **in sequence**:
   - `release-build` — the Windows x64 NSIS installer, signed, with a `latest.json` carrying the `windows-x86_64` entry.
   - `release-build-macos` — a universal (Apple Silicon + Intel) `.app` and `.dmg`, signed, merging the `darwin-aarch64` and `darwin-x86_64` entries into that *same* `latest.json`.
   - `release-build-linux` — an x64 `.AppImage`, signed, merging the `linux-x86_64` entry into it too. Built on Ubuntu 22.04 on purpose: an AppImage only runs on distros whose glibc is at least as new as the build machine's.

   They are sequenced on purpose: `tauri-action` rewrites the release's single `latest.json` asset, merging into whatever is already there, so running them concurrently would drop a platform. A failed job therefore also skips the ones after it.
3. Verify the release assets contain:
   - `French vACC Controller Pack Installer_<version>_x64-setup.exe`
   - `French vACC Controller Pack Installer_<version>_x64-setup.exe.sig`
   - `French vACC Controller Pack Installer_<version>_universal.dmg`
   - `French vACC Controller Pack Installer_universal.app.tar.gz`
   - `French vACC Controller Pack Installer_universal.app.tar.gz.sig`
   - `French vACC Controller Pack Installer_<version>_amd64.AppImage`
   - `French vACC Controller Pack Installer_<version>_amd64.AppImage.sig`
   - `latest.json` — open it and check it lists **four** platform keys: `windows-x86_64`, `darwin-aarch64`, `darwin-x86_64`, `linux-x86_64`. If one is missing, its job failed or was skipped; re-run it and confirm the merge before announcing the release.

The updater endpoint is `https://github.com/vaccfr/Sector-Files/releases/latest/download/latest.json` (configured in `tauri.conf.json` → `plugins.updater.endpoints`), so it always reads whatever the **latest** release published — there's no per-tag URL to update. End users with a previous Tauri installer (>= v0.1.0) see the new version offered the next time they launch the app (passive install on Windows; on macOS the updater swaps the `.app` in place and relaunches; on Linux it overwrites the `.AppImage` file itself, so the file must sit somewhere the user can write).

## Wine: where the pack goes on macOS and Linux

EuroScope itself is Windows-only, so off Windows it runs inside a Wine prefix (a folder containing `drive_c`), and the controller pack belongs in that same prefix. The installer looks in every prefix the common front-ends create:

| Front-end | Where its prefixes live |
| --- | --- |
| Plain Wine | `~/.wine`, `$WINEPREFIX`, and `~/.local/share/wineprefixes/*` (winetricks) |
| CrossOver | macOS: `~/Library/Application Support/CrossOver/Bottles/*`; Linux: `~/.cxoffice/*` |
| Whisky (macOS) | `~/Library/Containers/com.isaacmarovitz.Whisky/Bottles/*` |
| Kegworks, Sikarugir, Wineskin, Porting Kit (macOS) | wrapper apps in `~/Applications` (and its `Kegworks`/`Sikarugir`/`Wineskin` subfolders) |
| Bottles (Linux) | `~/.local/share/bottles/bottles/*`, or `~/.var/app/com.usebottles.bottles/data/bottles/bottles/*` for the Flatpak |
| Lutris (Linux) | `~/Games/*` |
| PlayOnLinux | `~/.PlayOnLinux/wineprefix/*` |

Prefixes that have EuroScope installed (`C:\Program Files (x86)\EuroScope\EuroScope.exe`) are preferred. On launch, an existing pack is detected in `~/Documents`, `~/Desktop`, `~`, or the `Documents` folder of any Windows user in any of those prefixes. When none is found, the Install tab lists the prefixes with an **Install here** button that proposes `<prefix>/drive_c/users/<user>/Documents/EuroScope`, and **Choose…** opens inside the best prefix. The directory can always be picked manually, and a prefix in a non-standard location just needs picking once.

vATIS is different: it has native macOS and Linux clients, so the installer uses those rather than a copy inside Wine, and writes the French vACC vATIS profiles to the native vATIS data folder (`~/Library/Application Support/org.vatsim.vatis` on macOS, `$XDG_DATA_HOME/org.vatsim.vatis` — normally `~/.local/share/org.vatsim.vatis` — on Linux).

## Installing the macOS build

Users install by opening the `.dmg` and dragging the app to `/Applications`.

**While the build is unsigned**, macOS quarantines it and the first launch fails with *"…is damaged and can't be opened"*. Tell users to either:

- right-click (or ⌃-click) the app → **Open** → **Open** in the dialog, or
- run once: `xattr -dr com.apple.quarantine "/Applications/French vACC Controller Pack Installer.app"`

This is only needed for the first launch; auto-updates afterwards are unaffected (the updater downloads without setting the quarantine flag). Configuring the Apple secrets above removes the workaround entirely.

## Installing the Linux build

The Linux build is a single x64 `.AppImage`: no installation, no root. Tell users to:

1. Put it somewhere they own, e.g. `~/Applications`. The auto-updater overwrites this file, so not a system folder.
2. Make it executable (`chmod +x "French vACC Controller Pack Installer_<version>_amd64.AppImage"`, or tick *Allow executing file as program* in the file manager's Properties) and run it.

AppImages need FUSE 2 to mount themselves. If nothing happens, or the error mentions `libfuse.so.2`, install it (`sudo apt install libfuse2t64` on Ubuntu 24.04+, `libfuse2` on older Ubuntu/Debian, `fuse-libs` on Fedora), or run the AppImage with `--appimage-extract-and-run`.

If vATIS is missing, the installer's vATIS step downloads the official vATIS AppImage to `~/Applications/vATIS.AppImage`, makes it executable and starts it. An existing vATIS AppImage is found in `~/Applications`, `~/AppImages`, `~/.local/bin`, `~/bin`, `~/Downloads` or `~/Desktop`, including one renamed by AppImageLauncher.

## Rolling back

If a release is broken in the wild:

1. Cut a follow-up release with a higher version that reverts the problematic change.
2. Do NOT delete the broken release — `latest.json` is updated on every release, so the broken version is naturally superseded. Deleting it would only matter for users intentionally pinning, which we don't support.

## Initial cutover from the legacy Python installer

The legacy Python `ControllerPackInstaller.exe` self-updated by hitting the same GitHub Releases stream. The first Tauri release (`v0.1.0`) should be accompanied by a one-time message pointing existing Python users at the new download URL — see `openspec/changes/rust-tauri-installer/design.md` §"Migration Plan".
