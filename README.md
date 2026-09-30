<img width="1900" height="450" alt="vACCFR_GitHub_Sector-Files" src="https://github.com/user-attachments/assets/7640ecba-b0d8-44a8-92f7-c9901c296146" />

# 🇫🇷 French vACC Sector File Repository

Welcome to the official collaborative repository for the **French vACC Controller Packs**.

This repository is designed to support **collaboration, maintenance, and issue tracking** for the controller packs used across French FIRs on VATSIM. It contains the **structure and configuration files** that make up each pack, ensuring consistency, realism, and compatibility for controllers.

---

## 📦 About This Repository

This repository includes core elements used in controller packs, such as:

- ⚙️ Settings files  
- 🗺️ ASRs
- 🔌 Plugins and supporting files  

These components are maintained here to streamline updates and allow community contributions.

> ⚠️ **Important:**  
> This repository does **not** contain the complete AIRAC sector packages.
> The Controller Pack Installer downloads those files from AeroNav GNG and
> combines them with the profiles, settings, displays, plugins and supporting
> resources maintained here.

---

## ⬇️ Install or Update the Controller Pack

Use the **Controller Pack Installer** — a signed desktop app for Windows, macOS and Linux that:

- Downloads and applies the latest configuration from this GitHub repository.
- Merges it with the AeroNav (GNG) AIRAC packages you download for your FIRs (see the [mirrors below](#raw-gng-package-mirrors)).
- Stores your CID, password, rating, and EuroScopeRPC preference once and applies them to every profile.
- Sets up the French vACC vATIS profiles after an install.
- Auto-updates itself via Tauri's signed updater channel.

Download it from the assets of the **[latest release](https://github.com/vaccfr/Sector-Files/releases/latest)**:

| Platform | File |
| --- | --- |
| Windows (x64) | `…_x64-setup.exe` |
| macOS (Apple Silicon & Intel) | `…_universal.dmg` |
| Linux (x64) | `…_amd64.AppImage` |

EuroScope is Windows-only, so on macOS and Linux it runs under Wine (CrossOver, Whisky, Bottles, Lutris, plain Wine…). The installer finds your Wine prefixes and puts the pack next to EuroScope. First-launch steps for the [macOS](installer/RELEASE.md#installing-the-macos-build) and [Linux](installer/RELEASE.md#installing-the-linux-build) builds are in the installer's release guide.

For maintainers cutting installer releases, see [installer/RELEASE.md](installer/RELEASE.md).

### Raw GNG package mirrors

If you need the raw AeroNav archives without the installer (e.g. for debugging), they live at:

- Bordeaux (LFBB): https://files.aero-nav.com/LFBB
- Reims & Paris (LFEE & LFFF) : https://files.aero-nav.com/LFXXN
- Marseille (LFMM): https://files.aero-nav.com/LFMM
- Brest (LFRR): https://files.aero-nav.com/LFRR

---

## 🐞 Bug Reporting

Spotted an issue?

- Incorrect frequency  
- Outdated sector drawings  
- Missing or broken features  
- General improvements or suggestions  

👉 Please open an issue here:  
https://github.com/vaccfr/Sector-Files/issues/new?template=sector-file-issue.md

Clear and detailed reports help us fix things faster.

---

## 🤝 Contributing

We welcome contributions from the community.

Before making a change, read [CONTRIBUTING.md](CONTRIBUTING.md) for the
repository structure, EuroScope file conventions, testing requirements,
automated maintenance and AIRAC process.

The usual workflow is:

1. Fork this repository.
2. Make and test a focused change.
3. Commit and push the change.
4. Open a pull request to `main` for review.

---

## 📌 Goal

The aim of this repository is to provide a **centralized, collaborative environment** to maintain high-quality controller packs for the French vACC, ensuring the best possible experience for both controllers and pilots.

---

Happy controlling! 🇫🇷✈️
