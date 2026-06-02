import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Toaster, toast } from "sonner";
import {
  AlertCircle,
  Download,
  FilePlus2,
  FolderOpen,
  Loader2,
  Package,
  RefreshCw,
  Save,
  X,
} from "lucide-react";
import {
  api,
  onEvent,
  type CheckUpdatesReport,
  type Profile,
  type SyncSummary,
} from "@/lib/tauri";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Checkbox } from "@/components/ui/checkbox";
import { Badge } from "@/components/ui/badge";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { UpdateBanner } from "@/components/UpdateBanner";

const RATINGS = ["OBS", "S1", "S2", "S3", "C1", "C2", "C3", "I1", "I2", "I3", "SUP", "ADM"] as const;

const basename = (p: string) => p.replace(/[\\/]+$/, "").split(/[\\/]/).pop() ?? p;

export default function App() {
  const [profile, setProfile] = useState<Profile | null>(null);
  const [updateStatus, setUpdateStatus] = useState<CheckUpdatesReport | null>(null);
  const [packages, setPackages] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [confirmOpen, setConfirmOpen] = useState(false);

  useEffect(() => {
    api.getProfile().then(async (p) => {
      if (!p.controller_pack_dir) {
        const detected = await api.detectPackDir();
        if (detected) p = await api.updateProfile({ controller_pack_dir: detected });
      }
      setProfile(p);
    });
    api.checkUpdates().then(setUpdateStatus).catch(() => {});

    let unlistenUpdates: (() => void) | undefined;
    let unlistenSync: (() => void) | undefined;
    onEvent<CheckUpdatesReport>("updates:report", setUpdateStatus).then((u) => (unlistenUpdates = u));
    onEvent<{ step: string }>("sync:progress", (p) => toast.loading(p.step, { id: "sync" })).then(
      (u) => (unlistenSync = u),
    );
    return () => {
      unlistenUpdates?.();
      unlistenSync?.();
    };
  }, []);

  const pickPackDir = useCallback(async () => {
    const dir = await open({ directory: true, multiple: false, title: "Select controller pack directory" });
    if (typeof dir === "string") {
      setProfile(await api.updateProfile({ controller_pack_dir: dir }));
    }
  }, []);

  const addPackages = useCallback(async () => {
    const picked = await open({
      multiple: true,
      title: "Select FIR package archives",
      filters: [{ name: "Controller pack archives", extensions: ["zip", "7z"] }],
    });
    if (!picked) return;
    const paths = Array.isArray(picked) ? picked : [picked];
    setPackages((prev) => Array.from(new Set([...prev, ...paths])));
  }, []);

  const removePackage = useCallback((path: string) => {
    setPackages((prev) => prev.filter((p) => p !== path));
  }, []);

  // Validate, then open the confirmation modal. The actual sync runs from the
  // modal's confirm button (runSync) so the user sees what's about to happen.
  const requestInstall = useCallback(() => {
    if (!profile?.controller_pack_dir) {
      toast.error("Set the controller pack directory first.");
      return;
    }
    if (packages.length === 0) {
      toast.error("Add at least one FIR package.");
      return;
    }
    setConfirmOpen(true);
  }, [profile, packages]);

  const runSync = useCallback(async () => {
    if (!profile?.controller_pack_dir) {
      toast.error("Set the controller pack directory first.");
      return;
    }
    if (packages.length === 0) {
      toast.error("Add at least one FIR package.");
      return;
    }
    setConfirmOpen(false);
    setBusy(true);
    try {
      const summary: SyncSummary = await api.runSync(packages);
      if (summary.warnings.length) {
        console.warn(`Sync warnings (${summary.warnings.length}):\n` + summary.warnings.join("\n"));
      }
      toast.success(`Sync complete — ${summary.files_written} file(s) written`, {
        id: "sync",
        description: summary.warnings.length ? `${summary.warnings.length} warning(s)` : undefined,
      });
      setProfile(await api.getProfile());
      api.checkUpdates().then(setUpdateStatus).catch(() => {});
    } catch (e) {
      toast.error("Sync failed", { id: "sync", description: String(e) });
    } finally {
      setBusy(false);
    }
  }, [profile, packages]);

  const refreshFromGithub = useCallback(async () => {
    if (!profile?.controller_pack_dir) {
      toast.error("Set the controller pack directory first.");
      return;
    }
    setBusy(true);
    toast.loading("Updating files from GitHub…", { id: "sync" });
    try {
      const summary: SyncSummary = await api.updateFromGithub();
      if (summary.warnings.length) {
        console.warn(`Update warnings (${summary.warnings.length}):\n` + summary.warnings.join("\n"));
      }
      toast.success(`Updated — ${summary.files_written} file(s) written`, {
        id: "sync",
        description: summary.warnings.length ? `${summary.warnings.length} warning(s)` : undefined,
      });
      setProfile(await api.getProfile());
      api.checkUpdates().then(setUpdateStatus).catch(() => {});
    } catch (e) {
      toast.error("Update failed", { id: "sync", description: String(e) });
    } finally {
      setBusy(false);
    }
  }, [profile]);

  if (!profile) {
    return (
      <div className="flex h-screen items-center justify-center text-neutral-400">
        <Loader2 className="mr-2 h-4 w-4 animate-spin" /> Loading…
      </div>
    );
  }

  return (
    <div className="flex h-screen flex-col">
      <UpdateBanner />
      <header className="flex items-center gap-3 border-b border-neutral-800 px-6 py-3">
        <Package className="h-5 w-5 text-brand" />
        <h1 className="text-base font-semibold">Controller Pack Installer</h1>
        <div className="ml-auto">
          <GithubBadge updates={updateStatus} />
        </div>
      </header>

      <main className="flex-1 overflow-y-auto px-6 py-6">
        <Tabs defaultValue="sync" className="mx-auto max-w-2xl space-y-6">
          <TabsList>
            <TabsTrigger value="sync">Install</TabsTrigger>
            <TabsTrigger value="profile">Profile</TabsTrigger>
            <TabsTrigger value="settings">Settings</TabsTrigger>
          </TabsList>

          <TabsContent value="sync">
            <SyncPanel
              profile={profile}
              packages={packages}
              busy={busy}
              onPickDir={pickPackDir}
              onAddPackages={addPackages}
              onRemovePackage={removePackage}
              onRun={requestInstall}
              onRefreshGithub={refreshFromGithub}
            />
          </TabsContent>
          <TabsContent value="profile">
            <ProfilePanel profile={profile} setProfile={setProfile} />
          </TabsContent>
          <TabsContent value="settings">
            <SettingsPanel profile={profile} setProfile={setProfile} />
          </TabsContent>
        </Tabs>
      </main>

      {confirmOpen && (
        <ConfirmInstallModal
          packages={packages}
          installDir={profile.controller_pack_dir ?? ""}
          onCancel={() => setConfirmOpen(false)}
          onConfirm={runSync}
        />
      )}

      <Toaster theme="dark" position="bottom-right" richColors closeButton />
    </div>
  );
}

function ConfirmInstallModal({
  packages,
  installDir,
  onCancel,
  onConfirm,
}: {
  packages: string[];
  installDir: string;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onCancel();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onCancel]);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4"
      role="dialog"
      aria-modal="true"
      onClick={onCancel}
    >
      <div
        className="max-h-[85vh] w-full max-w-lg overflow-y-auto rounded-lg border border-neutral-800 bg-neutral-950 shadow-xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center gap-2 border-b border-neutral-800 px-5 py-4">
          <Download className="h-5 w-5 text-brand" />
          <h2 className="text-base font-semibold">Before you install</h2>
        </div>

        <div className="space-y-4 px-5 py-4 text-sm text-neutral-300">
          <p>
            This merges the <strong>{packages.length}</strong> selected AIRAC package
            {packages.length === 1 ? "" : "s"} with the latest GitHub configuration into:
          </p>
          <p className="break-all rounded-md border border-neutral-800 bg-neutral-900 px-3 py-2 font-mono text-xs text-neutral-400">
            {installDir}
          </p>

          <ModalSection title="What gets changed">
            <ul className="list-disc space-y-1 pl-5 text-neutral-400">
              <li>
                GitHub-managed files (Settings, ASRs, <code>.prf</code> profiles, plugins, Alias)
                are overwritten with the latest versions.
              </li>
              <li>
                Sector files from your packages are renamed to <code>&lt;FIR&gt;.sct/.ese</code> and
                placed in <code>LFXX/Sectors</code>; ICAO &amp; NavData are copied in.
              </li>
              <li>
                Your VATSIM credentials are written into every <code>.prf</code> profile (when set).
              </li>
              <li>Files you added yourself that aren't part of the pack are left untouched.</li>
            </ul>
          </ModalSection>

          <ModalSection title="Choose every package you use">
            <p className="text-neutral-400">
              Only the FIRs whose packages you added are installed or updated. FIRs you leave out
              keep their current files — they are <strong>not</strong> removed, but they also won't
              receive this AIRAC's updates. To update everything, add a package for each FIR you
              fly.
            </p>
          </ModalSection>

          <ModalSection title="Backups are made automatically">
            <ul className="list-disc space-y-1 pl-5 text-neutral-400">
              <li>
                The previous sector files are moved to{" "}
                <code>LFXX/Sectors/Backup/&lt;FIR&gt;-&lt;airac&gt;.sct/.ese</code>.
              </li>
              <li>
                <code>LFXX/Settings</code> is snapshotted into <code>LFXX/Settings/backup</code>{" "}
                before it is overwritten.
              </li>
            </ul>
          </ModalSection>
        </div>

        <div className="flex justify-end gap-2 border-t border-neutral-800 px-5 py-4">
          <Button variant="outline" onClick={onCancel}>
            Cancel
          </Button>
          <Button onClick={onConfirm}>
            <Download className="h-4 w-4" /> Install / update
          </Button>
        </div>
      </div>
    </div>
  );
}

function ModalSection({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div className="space-y-1.5">
      <h3 className="font-medium text-neutral-200">{title}</h3>
      {children}
    </div>
  );
}

function GithubBadge({ updates }: { updates: CheckUpdatesReport | null }) {
  if (!updates) return <Badge variant="outline">checking…</Badge>;
  const g = updates.github;
  if (g.kind === "update_available") return <Badge variant="warning">GitHub update: {g.value}</Badge>;
  if (g.kind === "up_to_date") return <Badge variant="success">GitHub up to date</Badge>;
  return <Badge variant="outline">GitHub: ?</Badge>;
}

function SyncPanel({
  profile,
  packages,
  busy,
  onPickDir,
  onAddPackages,
  onRemovePackage,
  onRun,
  onRefreshGithub,
}: {
  profile: Profile;
  packages: string[];
  busy: boolean;
  onPickDir: () => void;
  onAddPackages: () => void;
  onRemovePackage: (p: string) => void;
  onRun: () => void;
  onRefreshGithub: () => void;
}) {
  return (
    <div className="space-y-6">
      <Card>
        <CardHeader>
          <CardTitle>Controller pack directory</CardTitle>
          <CardDescription>Where EuroScope's controller pack is installed.</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="flex gap-2">
            <Input value={profile.controller_pack_dir ?? ""} readOnly placeholder="Not set" />
            <Button variant="outline" onClick={onPickDir} className="shrink-0">
              <FolderOpen className="h-4 w-4" /> Choose…
            </Button>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>FIR packages</CardTitle>
          <CardDescription>
            Download the <code>.zip</code> / <code>.7z</code> packages for the FIRs you want from
            AeroNav, then add them here.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-3">
          {packages.length === 0 ? (
            <div className="flex flex-col items-center gap-2 rounded-lg border border-dashed border-neutral-800 py-8 text-neutral-500">
              <Package className="h-6 w-6" />
              <span className="text-sm">No packages selected yet.</span>
            </div>
          ) : (
            <ul className="space-y-1.5">
              {packages.map((p) => (
                <li
                  key={p}
                  className="flex items-center gap-2 rounded-md border border-neutral-800 bg-neutral-950 px-3 py-2 text-sm"
                >
                  <Package className="h-4 w-4 shrink-0 text-neutral-400" />
                  <span className="truncate" title={p}>
                    {basename(p)}
                  </span>
                  <button
                    onClick={() => onRemovePackage(p)}
                    className="ml-auto text-neutral-500 hover:text-red-400"
                    aria-label="Remove"
                  >
                    <X className="h-4 w-4" />
                  </button>
                </li>
              ))}
            </ul>
          )}
          <Button variant="secondary" onClick={onAddPackages}>
            <FilePlus2 className="h-4 w-4" /> Add packages…
          </Button>
        </CardContent>
      </Card>

      <div className="flex flex-col gap-3">
        <Button
          size="lg"
          onClick={onRun}
          disabled={busy || !profile.controller_pack_dir || packages.length === 0}
        >
          {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : <Download className="h-4 w-4" />}
          {busy ? "Installing…" : "Install / update"}
        </Button>
        <Button
          variant="outline"
          onClick={onRefreshGithub}
          disabled={busy || !profile.controller_pack_dir}
        >
          <RefreshCw className="h-4 w-4" /> Update GitHub files only
        </Button>
        <p className="flex items-start gap-2 text-xs text-neutral-500">
          <AlertCircle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
          <span>
            <strong>Install / update</strong> merges the selected AIRAC packages with the latest
            GitHub configuration. <strong>Update GitHub files only</strong> refreshes the
            GitHub-managed files for the FIRs you already have installed — no packages needed.
          </span>
        </p>
      </div>
    </div>
  );
}

function ProfilePanel({
  profile,
  setProfile,
}: {
  profile: Profile;
  setProfile: (p: Profile) => void;
}) {
  const [draft, setDraft] = useState(profile.vatsim);
  useEffect(() => setDraft(profile.vatsim), [profile.vatsim]);
  const dirty = useMemo(
    () => JSON.stringify(draft) !== JSON.stringify(profile.vatsim),
    [draft, profile.vatsim],
  );

  const save = async () => {
    setProfile(await api.updateProfile({ vatsim: draft }));
    toast.success("Profile saved");
  };

  const applyNow = async () => {
    if (!profile.controller_pack_dir) return toast.error("Set controller pack directory first");
    const count = await api.applyProfileToPack(profile.controller_pack_dir);
    toast.success(`Patched ${count} file(s)`);
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle>VATSIM profile</CardTitle>
        <CardDescription>Written into the controller pack's EuroScope profiles.</CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="rounded-md border border-amber-700/40 bg-amber-700/10 px-3 py-2 text-xs text-amber-200">
          Credentials are stored unencrypted on disk. EuroScope also writes the VATSIM password in
          plain text into every <code>.prf</code> file.
        </div>
        <Field label="Real name">
          <Input value={draft.real_name} onChange={(e) => setDraft({ ...draft, real_name: e.target.value })} />
        </Field>
        <Field label="VATSIM CID">
          <Input
            value={draft.cid}
            placeholder="1234567"
            onChange={(e) => setDraft({ ...draft, cid: e.target.value })}
          />
        </Field>
        <Field label="Password">
          <Input
            type="password"
            value={draft.password}
            onChange={(e) => setDraft({ ...draft, password: e.target.value })}
          />
        </Field>
        <Field label="Rating">
          <Select value={draft.rating} onValueChange={(v) => setDraft({ ...draft, rating: v })}>
            <SelectTrigger className="w-40">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {RATINGS.map((r) => (
                <SelectItem key={r} value={r}>
                  {r}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </Field>
        <label className="flex items-center gap-2 text-sm">
          <Checkbox
            checked={draft.enable_rpc}
            onCheckedChange={(c) => setDraft({ ...draft, enable_rpc: c === true })}
          />
          Enable Discord Rich Presence (EuroScopeRPC) plugin
        </label>
        <div className="flex gap-2 pt-1">
          <Button onClick={save} disabled={!dirty}>
            <Save className="h-4 w-4" /> Save
          </Button>
          <Button variant="outline" onClick={applyNow} disabled={!profile.controller_pack_dir}>
            Apply now to installed pack
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}

function SettingsPanel({
  profile,
  setProfile,
}: {
  profile: Profile;
  setProfile: (p: Profile) => void;
}) {
  const togglePref = async (key: "auto_check_updates" | "apply_creds_after_sync") => {
    const prefs = { ...profile.preferences, [key]: !profile.preferences[key] };
    setProfile(await api.updateProfile({ preferences: prefs }));
  };

  return (
    <div className="space-y-6">
      <Card>
        <CardHeader>
          <CardTitle>Preferences</CardTitle>
        </CardHeader>
        <CardContent className="space-y-3">
          <label className="flex items-center gap-2 text-sm">
            <Checkbox
              checked={profile.preferences.apply_creds_after_sync}
              onCheckedChange={() => togglePref("apply_creds_after_sync")}
            />
            Apply credentials to <code>.prf</code> files after install
          </label>
          <label className="flex items-center gap-2 text-sm">
            <Checkbox
              checked={profile.preferences.auto_check_updates}
              onCheckedChange={() => togglePref("auto_check_updates")}
            />
            Auto-check for updates every 30 minutes
          </label>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Installed versions</CardTitle>
        </CardHeader>
        <CardContent className="flex gap-2">
          <Badge variant="outline">GitHub: {profile.versions.installed_github_sha ?? "—"}</Badge>
          <Badge variant="outline">AIRAC: {profile.versions.installed_airac_cycle ?? "—"}</Badge>
        </CardContent>
      </Card>
    </div>
  );
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="grid grid-cols-[160px_1fr] items-center gap-3">
      <Label>{label}</Label>
      <div>{children}</div>
    </div>
  );
}
