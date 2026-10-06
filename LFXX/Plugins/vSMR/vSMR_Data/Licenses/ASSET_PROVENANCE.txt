# Bundled asset provenance register

This register prevents bundled assets from being mistaken for original vSMR
code or automatically covered by the project's GPL license. It is deliberately
conservative: an entry marked **verification required** must be resolved by the
release owner before a public production release.

`create_release_package.ps1` enforces this register: a publishable package is
refused while any entry remains unresolved. `-ForceNonPublishable` exists only
for local validation and must not be used to distribute those assets.
The release-status column accepts `Project license`, `License verified`,
`Permission documented`, `Public domain`, `Original asset`, or
`Verification required`. Missing, malformed, empty, or unknown entries also
stop publishable packaging.

| Asset group | Packaged path | Current provenance record | Release status |
| --- | --- | --- | --- |
| Aircraft silhouettes | `aircraft_icons/*.png` | Imported in commit `8b9cdaed37997a0e35ce48426938260a475feab0` (2026-07-04); individual creator and upstream redistribution terms are not recorded | Verification required |
| Timer alarm | `Audio/Alarm.wav` | Restored from the historical ESTimers installation in commit `f462c00009d9e27732b8e0507efca8c867ee2b0a` (2026-08-21); creator and redistribution terms are not recorded | Verification required |
| CPDLC notification | `Audio/Ding.wav` | Unmodified `Ding.wav` from pierr3/vSMR, commit `e696e0932af07cfa16068ffbb843ad13c7882266`, under that repository's GPL-3.0 license; identical Git blob `b87a6b75c2380c5c40a85cb7fae15a90c8e4d93d`, checked 2026-09-29. See evidence below; GPL text is shipped as `Licenses/vSMR.txt` | License verified |
| AVISO airport geometry | `AVISO/<ICAO>.geojson` | 160 airport maps refreshed from dev PR #34 (commit `4819ea1`) on 2026-09-25, using official GNG layouts and current settings. LFPG retains its 89 East and 97 West arrow lines in six grouped MultiLineString features. Current hashes are recorded in `vSMR/tests/fixtures/aviso_inventory.json`; earlier imports and historical arrow provenance remain in Git history. Upstream GPL-3.0 license retained as `France-Ground-Layouts.txt`; local and historical source redistribution terms still require review | Verification required |
| Aircraft dimensions | `ICAO_Aircraft.json` | Present as `vSMR/ICAO_Aircraft.json` in initial commit `5dd2cb6`, reorganized in `e9ec68b` and normalized in `5fab665`; upstream database source and terms are not recorded | Verification required |
| EuroScope Plugin Bridge client shim | `Runtime/vSMR.Runtime.dll` (compiled from `lib/include/esbridge.h`) | Unmodified header from AlexisBalzano/Euroscope-Plugin-Bridge commit `be6e0de6d3358e63c7cea0308ffcc16c87a61f8c`; upstream repository has no license file as checked 2026-09-29. Header redistribution/compilation permission must be documented even though the bridge DLL is not bundled | Verification required |
| Control Center UI | `src/control_center/web/*` (packaged as `vSMR_webUI/*`) | Maintained as part of this repository | Project license |

For each unresolved group, record the source URL or contributor, retrieval or
creation date, applicable license/permission, and any required attribution.
Do not remove this notice merely because a file can be downloaded publicly.

## Evidence reviewed on 2026-09-29

- CPDLC sound: [original file](https://github.com/pierr3/vSMR/blob/e696e0932af07cfa16068ffbb843ad13c7882266/Ding.wav) and [repository GPL-3.0 license](https://github.com/pierr3/vSMR/blob/e696e0932af07cfa16068ffbb843ad13c7882266/LICENSE). `git hash-object vSMR/data/Audio/Ding.wav` matches the upstream Git blob above. Attribution: Pierre Ferran and the original vSMR contributors.
- The [France-Ground-Layouts repository](https://github.com/vaccfr/France-Ground-Layouts) publishes a GPL-3.0 license, retained in this package. This does not by itself establish the source of every historical/local addition, including the restored LFPG arrows and supplied LFLL detail geometry; the combined AVISO group remains unresolved.
- [EuroScope Plugin Bridge](https://github.com/AlexisBalzano/Euroscope-Plugin-Bridge) has no published repository license. Its client shim is compiled into the runtime, so it is included in this release gate as well as the dependency inventory.

Outstanding evidence must identify the original source and applicable license or
permission for each unresolved group. Do not turn an unresolved entry into
`Project license` solely because it has been committed to this repository.
