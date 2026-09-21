import { useEffect, useState } from "react";
import { DiagnosticsSection } from "@/components/DiagnosticsSection";
import { RetroArchSettings } from "@/components/RetroArchSettings";
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
import type { Config, ProviderKind, RetroArchInput } from "@/lib/types";

const DEFAULT_RETROARCH_INPUT: RetroArchInput = {
  up: "space",
  down: "s",
  left: "a",
  right: "d",
  lightPunch: "u",
  mediumPunch: "i",
  heavyPunch: "o",
  lightKick: "j",
  mediumKick: "k",
  heavyKick: "l",
  start: "num1",
  coin: "num5",
};

interface SettingsDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  config: Config | null;
  onSave: (config: Config) => Promise<void>;
}

interface FormState {
  handle: string;
  fightcadeDir: string;
  romDir: string;
  tailscalePath: string;
  defaultPeerIp: string;
  rttWarnMs: string;
  pollIntervalSecs: string;
  cabinetMode: boolean;
  provider: ProviderKind;
  retroarchPath: string;
  retroarchCore: string;
  retroarchPort: string;
  retroarchCommandPort: string;
  retroarchNickname: string;
  retroarchMuteSpectators: boolean;
  retroarchMaxPingMs: string;
  retroarchIsolatedConfig: boolean;
  retroarchInput: RetroArchInput;
  retroarchInputEnabled: boolean;
  verboseLogging: boolean;
  developerMode: boolean;
}

function toForm(config: Config | null): FormState {
  return {
    handle: config?.handle ?? "",
    fightcadeDir: config?.fightcadeDir ?? "",
    romDir: config?.romDir ?? "",
    tailscalePath: config?.tailscalePath ?? "",
    defaultPeerIp: config?.defaultPeerIp ?? "",
    rttWarnMs: String(config?.rttWarnMs ?? 150),
    pollIntervalSecs: String(config?.pollIntervalSecs ?? 10),
    cabinetMode: config?.cabinetMode ?? false,
    provider: config?.provider ?? "fightcade",
    retroarchPath: config?.retroarchPath ?? "",
    retroarchCore: config?.retroarchCore ?? "",
    retroarchPort: String(config?.retroarchPort ?? 55435),
    retroarchCommandPort: String(config?.retroarchCommandPort ?? 55355),
    retroarchNickname: config?.retroarchNickname ?? "",
    retroarchMuteSpectators: config?.retroarchMuteSpectators ?? true,
    retroarchMaxPingMs: String(config?.retroarchMaxPingMs ?? 0),
    retroarchIsolatedConfig: config?.retroarchIsolatedConfig ?? false,
    retroarchInput: config?.retroarchInput ?? DEFAULT_RETROARCH_INPUT,
    retroarchInputEnabled: config?.retroarchInputEnabled ?? true,
    verboseLogging: config?.verboseLogging ?? false,
    developerMode: config?.developerMode ?? false,
  };
}

const emptyToNull = (value: string) => (value.trim() === "" ? null : value.trim());

function buildConfig(form: FormState): Config {
  return {
    handle: emptyToNull(form.handle),
    fightcadeDir: emptyToNull(form.fightcadeDir),
    romDir: emptyToNull(form.romDir),
    tailscalePath: emptyToNull(form.tailscalePath),
    defaultPeerIp: emptyToNull(form.defaultPeerIp),
    rttWarnMs: Number(form.rttWarnMs) || 150,
    pollIntervalSecs: Math.max(2, Number(form.pollIntervalSecs) || 10),
    cabinetMode: form.cabinetMode,
    provider: form.provider,
    retroarchPath: emptyToNull(form.retroarchPath),
    retroarchCore: emptyToNull(form.retroarchCore),
    retroarchPort: Number(form.retroarchPort) || 55435,
    retroarchCommandPort: Number(form.retroarchCommandPort) || 55355,
    retroarchNickname: emptyToNull(form.retroarchNickname),
    retroarchMuteSpectators: form.retroarchMuteSpectators,
    retroarchMaxPingMs: Math.max(0, Number(form.retroarchMaxPingMs) || 0),
    retroarchIsolatedConfig: form.retroarchIsolatedConfig,
    retroarchInput: form.retroarchInput,
    retroarchInputEnabled: form.retroarchInputEnabled,
    verboseLogging: form.verboseLogging,
    developerMode: form.developerMode,
  };
}

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

  const setField = (key: keyof FormState, value: string) =>
    setForm((prev) => ({ ...prev, [key]: value }));

  const setToggle = (key: keyof FormState, value: boolean) =>
    setForm((prev) => ({ ...prev, [key]: value }));

  const setInput = (key: keyof RetroArchInput, value: string) =>
    setForm((prev) => ({
      ...prev,
      retroarchInput: { ...prev.retroarchInput, [key]: value },
    }));

  async function handleSave() {
    setSaving(true);
    try {
      await onSave(buildConfig(form));
      onOpenChange(false);
    } finally {
      setSaving(false);
    }
  }

  async function handleCoreDownloaded(path: string) {
    const next = { ...form, retroarchCore: path };
    setForm(next);
    await onSave(buildConfig(next));
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
          </div>
          <div className="flex items-center justify-between gap-4">
            <div className="grid gap-1">
              <Label htmlFor="developerMode">Developer mode</Label>
              <p className="text-xs text-muted-foreground">
                Show developer tools on the home screen, like the loopback Dev pair.
              </p>
            </div>
            <input
              id="developerMode"
              type="checkbox"
              className="size-4 shrink-0 accent-primary"
              checked={form.developerMode}
              onChange={(event) =>
                setForm((prev) => ({
                  ...prev,
                  developerMode: event.target.checked,
                }))
              }
            />
          </div>
          <div className="grid gap-2">
            <Label htmlFor="handle">Default nickname</Label>
            <Input
              id="handle"
              value={form.handle}
              placeholder="Fallback when netplay nickname is unset"
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
            <RetroArchSettings
              path={form.retroarchPath}
              core={form.retroarchCore}
              port={form.retroarchPort}
              commandPort={form.retroarchCommandPort}
              nickname={form.retroarchNickname}
              handle={form.handle}
              maxPingMs={form.retroarchMaxPingMs}
              muteSpectators={form.retroarchMuteSpectators}
              isolatedConfig={form.retroarchIsolatedConfig}
              input={form.retroarchInput}
              inputEnabled={form.retroarchInputEnabled}
              onChange={setField}
              onToggle={setToggle}
              onInputChange={setInput}
              onInputReplace={(input) =>
                setForm((prev) => ({ ...prev, retroarchInput: input }))
              }
              onCoreDownloaded={handleCoreDownloaded}
            />
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

          <DiagnosticsSection
            verboseLogging={form.verboseLogging}
            onVerboseLoggingChange={(value) =>
              setForm((prev) => ({ ...prev, verboseLogging: value }))
            }
          />
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
