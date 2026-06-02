import { useEffect, useState } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { Button } from "@/components/ui/button";

const BACKOFF_KEY = "installer-update-last-check";
const BACKOFF_MS = 24 * 60 * 60 * 1000;

export function UpdateBanner() {
  const [update, setUpdate] = useState<Update | null>(null);
  const [installing, setInstalling] = useState(false);
  const [dismissed, setDismissed] = useState(false);

  useEffect(() => {
    const last = Number(localStorage.getItem(BACKOFF_KEY) ?? 0);
    if (Date.now() - last < BACKOFF_MS) return;

    check()
      .then((u) => {
        localStorage.setItem(BACKOFF_KEY, String(Date.now()));
        if (u) setUpdate(u);
      })
      .catch(() => {});
  }, []);

  if (!update || dismissed) return null;

  const install = async () => {
    setInstalling(true);
    try {
      await update.downloadAndInstall();
      await relaunch();
    } catch (e) {
      console.error("update install failed", e);
      setInstalling(false);
    }
  };

  return (
    <div className="bg-amber-600/15 border-b border-amber-700/40 px-4 py-2 text-sm flex items-center gap-3">
      <span className="flex-1">
        Installer update available: <strong>{update.version}</strong>
      </span>
      <Button size="sm" onClick={install} disabled={installing}>
        {installing ? "Installing…" : "Install update"}
      </Button>
      <Button size="sm" variant="ghost" onClick={() => setDismissed(true)} disabled={installing}>
        Later
      </Button>
    </div>
  );
}
