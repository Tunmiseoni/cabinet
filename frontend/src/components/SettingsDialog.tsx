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
import { Separator } from "@/components/ui/separator";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { Config, ProviderKind } from "@/lib/api";

interface SettingsDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  config: Config | null;
  onSave: (config: Config) => Promise<void>;
  onResetScores: () => Promise<void>;
}

interface FormState {
  handle: string;
  fightcadeDir: string;
  romDir: string;
  tailscalePath: string;
  defaultPeerIp: string;
  discoveryPort: string;
  controlPort: string;
  rttWarnMs: string;
  pollIntervalSecs: string;
  cabinetMode: boolean;
  provider: ProviderKind;
  retroarchPath: string;
  retroarchCore: string;
  retroarchPort: string;
  retroarchNickname: string;
}

function toForm(config: Config | null): FormState {
  return {
    handle: config?.handle ?? "",
    fightcadeDir: config?.fightcadeDir ?? "",
    romDir: config?.romDir ?? "",
    tailscalePath: config?.tailscalePath ?? "",
    defaultPeerIp: config?.defaultPeerIp ?? "",
    discoveryPort: String(config?.discoveryPort ?? 47810),
    controlPort: String(config?.controlPort ?? 47811),
    rttWarnMs: String(config?.rttWarnMs ?? 150),
    pollIntervalSecs: String(config?.pollIntervalSecs ?? 10),
    cabinetMode: config?.cabinetMode ?? false,
    provider: config?.provider ?? "fightcade",
    retroarchPath: config?.retroarchPath ?? "",
    retroarchCore: config?.retroarchCore ?? "",
    retroarchPort: String(config?.retroarchPort ?? 55435),
    retroarchNickname: config?.retroarchNickname ?? "",
  };
}

const emptyToNull = (value: string) => (value.trim() === "" ? null : value.trim());

export function SettingsDialog({
  open,
  onOpenChange,
  config,
  onSave,
  onResetScores,
}: SettingsDialogProps) {
  const [form, setForm] = useState<FormState>(() => toForm(config));
  const [saving, setSaving] = useState(false);
  const [confirmingReset, setConfirmingReset] = useState(false);

  useEffect(() => {
    if (open) setForm(toForm(config));
  }, [open, config]);

  useEffect(() => {
    if (!open) setConfirmingReset(false);
  }, [open]);

  const update = (key: keyof FormState) => (value: string) =>
    setForm((prev) => ({ ...prev, [key]: value }));

  async function handleReset() {
    await onResetScores();
    setConfirmingReset(false);
  }

  async function handleSave() {
    setSaving(true);
    try {
      await onSave({
        handle: emptyToNull(form.handle),
        fightcadeDir: emptyToNull(form.fightcadeDir),
        romDir: emptyToNull(form.romDir),
        tailscalePath: emptyToNull(form.tailscalePath),
        defaultPeerIp: emptyToNull(form.defaultPeerIp),
        discoveryPort: Number(form.discoveryPort) || 47810,
        controlPort: Number(form.controlPort) || 47811,
        rttWarnMs: Number(form.rttWarnMs) || 150,
        pollIntervalSecs: Math.max(2, Number(form.pollIntervalSecs) || 10),
        cabinetMode: form.cabinetMode,
        provider: form.provider,
        retroarchPath: emptyToNull(form.retroarchPath),
        retroarchCore: emptyToNull(form.retroarchCore),
        retroarchPort: Number(form.retroarchPort) || 55435,
        retroarchNickname: emptyToNull(form.retroarchNickname),
      });
      onOpenChange(false);
    } finally {
      setSaving(false);
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="flex max-h-[calc(100dvh-2rem)] flex-col gap-4 sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>Settings</DialogTitle>
          <DialogDescription>
            Overrides for install paths and nethealth thresholds. Leave a field
            blank to use the platform default.
          </DialogDescription>
        </DialogHeader>
        <div className="-mr-4 grid min-h-0 flex-1 gap-4 overflow-y-auto pr-4">
          <div className="grid gap-2">
            <Label htmlFor="provider">Match provider</Label>
            <Select
              value={form.provider}
              onValueChange={(value) =>
                setForm((prev) => ({ ...prev, provider: value as ProviderKind }))
              }
            >
              <SelectTrigger id="provider">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="fightcade">FightCade (Wine, quark:direct)</SelectItem>
                <SelectItem value="retroarch">RetroArch (native netplay)</SelectItem>
              </SelectContent>
            </Select>
            <p className="text-xs text-muted-foreground">
              RetroArch has no overlay results, so matches are reported manually.
            </p>
          </div>
          <div className="grid gap-2">
            <Label htmlFor="handle">Your handle</Label>
            <Input
              id="handle"
              value={form.handle}
              placeholder="Shown in rooms and the ladder (defaults to your tailnet hostname)"
              onChange={(event) => update("handle")(event.target.value)}
            />
          </div>
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
          {form.provider === "retroarch" && (
            <>
              <div className="grid gap-2">
                <Label htmlFor="retroarchPath">RetroArch binary</Label>
                <Input
                  id="retroarchPath"
                  value={form.retroarchPath}
                  placeholder="/Applications/RetroArch.app/Contents/MacOS/RetroArch"
                  onChange={(event) =>
                    update("retroarchPath")(event.target.value)
                  }
                />
              </div>
              <div className="grid gap-2">
                <Label htmlFor="retroarchCore">FBNeo core (frozen)</Label>
                <Input
                  id="retroarchCore"
                  value={form.retroarchCore}
                  placeholder="/Users/you/Library/Application Support/RetroArch/cores/fbneo_libretro.dylib"
                  onChange={(event) =>
                    update("retroarchCore")(event.target.value)
                  }
                />
                <p className="text-xs text-muted-foreground">
                  Leave blank to auto-detect the standard RetroArch core folder.
                  Netplay refuses to sync if every machine's core revision does
                  not match.
                </p>
              </div>
              <div className="grid gap-4 sm:grid-cols-2">
                <div className="grid gap-2">
                  <Label htmlFor="retroarchPort">Netplay port</Label>
                  <Input
                    id="retroarchPort"
                    type="number"
                    min={1}
                    max={65535}
                    value={form.retroarchPort}
                    onChange={(event) =>
                      update("retroarchPort")(event.target.value)
                    }
                  />
                </div>
                <div className="grid gap-2">
                  <Label htmlFor="retroarchNickname">Netplay nickname</Label>
                  <Input
                    id="retroarchNickname"
                    value={form.retroarchNickname}
                    placeholder={form.handle || "your handle"}
                    onChange={(event) =>
                      update("retroarchNickname")(event.target.value)
                    }
                  />
                </div>
              </div>
            </>
          )}
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
          <div className="grid gap-4 sm:grid-cols-2">
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
          </div>
          <div className="grid gap-4 sm:grid-cols-2">
            <div className="grid gap-2">
              <Label htmlFor="discoveryPort">Discovery port (UDP)</Label>
              <Input
                id="discoveryPort"
                type="number"
                min={1}
                max={65535}
                value={form.discoveryPort}
                onChange={(event) => update("discoveryPort")(event.target.value)}
              />
            </div>
            <div className="grid gap-2">
              <Label htmlFor="controlPort">Control port (TCP)</Label>
              <Input
                id="controlPort"
                type="number"
                min={1}
                max={65535}
                value={form.controlPort}
                onChange={(event) => update("controlPort")(event.target.value)}
              />
            </div>
          </div>

          <Separator />

          <div className="flex items-center justify-between gap-4">
            <div className="grid gap-1">
              <Label htmlFor="cabinetMode">Cabinet mode</Label>
              <p className="text-xs text-muted-foreground">
                Host the emulator inside The Cabinet during a match (macOS). Needs
                Accessibility permission; without it the game stays a separate window.
              </p>
            </div>
            <input
              id="cabinetMode"
              type="checkbox"
              className="size-4 shrink-0 accent-primary"
              checked={form.cabinetMode}
              onChange={(event) =>
                setForm((prev) => ({ ...prev, cabinetMode: event.target.checked }))
              }
            />
          </div>

          <Separator />

          <div className="flex items-center justify-between gap-4">
            <div className="grid gap-1">
              <Label>Lifetime scores</Label>
              <p className="text-xs text-muted-foreground">
                Erase the per-opponent win/loss history. This cannot be undone.
              </p>
            </div>
            {confirmingReset ? (
              <div className="flex shrink-0 items-center gap-2">
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => setConfirmingReset(false)}
                >
                  Cancel
                </Button>
                <Button
                  variant="destructive"
                  size="sm"
                  onClick={() => void handleReset()}
                >
                  Confirm reset
                </Button>
              </div>
            ) : (
              <Button
                variant="outline"
                size="sm"
                className="shrink-0"
                onClick={() => setConfirmingReset(true)}
              >
                Reset scores
              </Button>
            )}
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
