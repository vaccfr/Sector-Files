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

## Cutting a release

The pack ships as a **single combined release roughly once a month**. Any published GitHub Release — whether it carries AIRAC (sector) changes, installer changes, or both — triggers `.github/workflows/build-installer-tauri.yml`, which rebuilds, signs, and attaches the Windows x64 installer plus `latest.json` to that same release. There is no dedicated installer-only release stream and no special tag prefix; the workflow keys off the release event, not the tag name.

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
2. The `release-build` job runs only on `release: published`. It builds the Windows x64 NSIS installer with `tauri-action`, signs it, generates `latest.json`, and attaches everything to the release. (Linux is only a smoke build on non-release pushes/PRs — no Linux artifact is published.)
3. Verify the release assets contain:
   - `French vACC Controller Pack Installer_<version>_x64-setup.exe`
   - `French vACC Controller Pack Installer_<version>_x64-setup.exe.sig`
   - `latest.json`

The updater endpoint is `https://github.com/vaccfr/Sector-Files/releases/latest/download/latest.json` (configured in `tauri.conf.json` → `plugins.updater.endpoints`), so it always reads whatever the **latest** release published — there's no per-tag URL to update. End users with a previous Tauri installer (>= v0.1.0) see the new version offered the next time they launch the app (passive install on Windows).

## Rolling back

If a release is broken in the wild:

1. Cut a follow-up release with a higher version that reverts the problematic change.
2. Do NOT delete the broken release — `latest.json` is updated on every release, so the broken version is naturally superseded. Deleting it would only matter for users intentionally pinning, which we don't support.

## Initial cutover from the legacy Python installer

The legacy Python `ControllerPackInstaller.exe` self-updated by hitting the same GitHub Releases stream. The first Tauri release (`v0.1.0`) should be accompanied by a one-time message pointing existing Python users at the new download URL — see `openspec/changes/rust-tauri-installer/design.md` §"Migration Plan".
