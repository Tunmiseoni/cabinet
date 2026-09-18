import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import type { Config } from "@/lib/api";

interface SettingsDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  config: Config | null;
  onSave: (config: Config) => Promise<void>;
}

interface FormState {
  fightcadeDir: string;
  romDir: string;
  tailscalePath: string;
  defaultPeerIp: string;
  rttWarnMs: string;
  pollIntervalSecs: string;
}

function toForm(config: Config | null): FormState {
  return {
    fightcadeDir: config?.fightcadeDir ?? "",
    romDir: config?.romDir ?? "",
    tailscalePath: config?.tailscalePath ?? "",
    defaultPeerIp: config?.defaultPeerIp ?? "",
    rttWarnMs: String(config?.rttWarnMs ?? 150),
    pollIntervalSecs: String(config?.pollIntervalSecs ?? 10),
  };
}

const emptyToNull = (value: string) => (value.trim() === "" ? null : value.trim());

export function SettingsDialog({
  open,
  onOpenChange,
  config,
  onSave,
}: SettingsDialogProps) {
  const [form, setForm] = useState<FormState>(() => toForm(config));
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (open) setForm(toForm(config));
  }, [open, config]);

  const update = (key: keyof FormState) => (value: string) =>
    setForm((prev) => ({ ...prev, [key]: value }));

  async function handleSave() {
    setSaving(true);
    try {
      await onSave({
        fightcadeDir: emptyToNull(form.fightcadeDir),
        romDir: emptyToNull(form.romDir),
        tailscalePath: emptyToNull(form.tailscalePath),
        defaultPeerIp: emptyToNull(form.defaultPeerIp),
        rttWarnMs: Number(form.rttWarnMs) || 150,
        pollIntervalSecs: Math.max(2, Number(form.pollIntervalSecs) || 10),
      });
      onOpenChange(false);
    } finally {
      setSaving(false);
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>Settings</DialogTitle>
          <DialogDescription>
            Overrides for install paths and nethealth thresholds. Leave a field
            blank to use the platform default.
          </DialogDescription>
        </DialogHeader>
        <div className="grid gap-4">
          <div className="grid gap-2">
            <Label htmlFor="romDir">ROM directory</Label>
            <Input
              id="romDir"
              value={form.romDir}
              placeholder="/Applications/FightCade2.app/.../fbneo/ROMs"
              onChange={(event) => update("romDir")(event.target.value)}
            />
          </div>
          <div className="grid gap-2">
            <Label htmlFor="fightcadeDir">FightCade install directory</Label>
            <Input
              id="fightcadeDir"
              value={form.fightcadeDir}
              placeholder="/Applications/FightCade2.app"
              onChange={(event) => update("fightcadeDir")(event.target.value)}
            />
          </div>
          <div className="grid gap-2">
            <Label htmlFor="tailscalePath">Tailscale binary</Label>
            <Input
              id="tailscalePath"
              value={form.tailscalePath}
              placeholder="/Applications/Tailscale.app/Contents/MacOS/Tailscale"
              onChange={(event) => update("tailscalePath")(event.target.value)}
            />
          </div>
          <div className="grid gap-2">
            <Label htmlFor="defaultPeerIp">Default peer IP</Label>
            <Input
              id="defaultPeerIp"
              value={form.defaultPeerIp}
              placeholder="100.x.x.x"
              onChange={(event) => update("defaultPeerIp")(event.target.value)}
            />
          </div>
          <div className="grid grid-cols-2 gap-4">
            <div className="grid gap-2">
              <Label htmlFor="rttWarnMs">Ping warning threshold (ms)</Label>
              <Input
                id="rttWarnMs"
                type="number"
                min={20}
                value={form.rttWarnMs}
                onChange={(event) => update("rttWarnMs")(event.target.value)}
              />
            </div>
            <div className="grid gap-2">
              <Label htmlFor="pollIntervalSecs">Poll interval (s)</Label>
              <Input
                id="pollIntervalSecs"
                type="number"
                min={2}
                value={form.pollIntervalSecs}
                onChange={(event) =>
                  update("pollIntervalSecs")(event.target.value)
                }
              />
            </div>
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button onClick={handleSave} disabled={saving}>
            {saving ? "Saving…" : "Save"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
