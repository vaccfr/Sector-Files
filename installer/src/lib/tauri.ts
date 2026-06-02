import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type FirCode = "LFBB" | "LFEE" | "LFFF" | "LFMM" | "LFRR";

export interface VatsimCredentials {
  cid: string;
  password: string;
  rating: string;
  real_name: string;
  enable_rpc: boolean;
}

export interface InstalledVersions {
  installed_github_sha: string | null;
  installed_airac_cycle: string | null;
}

export interface Preferences {
  auto_check_updates: boolean;
  apply_creds_after_sync: boolean;
}

export interface Profile {
  controller_pack_dir: string | null;
  vatsim: VatsimCredentials;
  versions: InstalledVersions;
  preferences: Preferences;
}

export interface ProfilePatch {
  controller_pack_dir?: string | null;
  vatsim?: VatsimCredentials;
  versions?: InstalledVersions;
  preferences?: Preferences;
}

export interface SyncSummary {
  github_sha: string | null;
  airac_cycle: string | null;
  files_written: number;
  files_skipped: number;
  warnings: string[];
}

export type CheckStatus =
  | { kind: "up_to_date" }
  | { kind: "update_available"; value: string }
  | { kind: "unknown"; reason: string };

export interface CheckUpdatesReport {
  github: CheckStatus;
  airac: CheckStatus;
}

export interface InstallerUpdateReport {
  available: boolean;
  latest_version: string | null;
  current_version: string;
}

export const api = {
  getProfile: () => invoke<Profile>("get_profile"),
  updateProfile: (patch: ProfilePatch) => invoke<Profile>("update_profile", { patch }),
  detectPackDir: () => invoke<string | null>("detect_pack_dir"),
  looksLikeControllerPack: (path: string) =>
    invoke<boolean>("looks_like_controller_pack", { path }),
  runSync: (packagePaths: string[], alsoApplyProfile?: boolean) =>
    invoke<SyncSummary>("run_sync", { packagePaths, alsoApplyProfile }),
  updateFromGithub: (alsoApplyProfile?: boolean) =>
    invoke<SyncSummary>("update_from_github", { alsoApplyProfile }),
  applyProfileToPack: (installRoot: string) =>
    invoke<number>("apply_profile_to_pack", { installRoot }),
  importPluginLines: (installRoot: string, examplePrf: string) =>
    invoke<number>("import_plugin_lines", { installRoot, examplePrf }),
  checkUpdates: () => invoke<CheckUpdatesReport>("check_updates"),
  checkInstallerUpdate: () => invoke<InstallerUpdateReport>("check_installer_update"),
};

export async function onEvent<T>(
  event: string,
  handler: (payload: T) => void,
): Promise<UnlistenFn> {
  return listen<T>(event, (e) => handler(e.payload));
}
