import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { getRetroArchHotkeys, downloadRetroArchCore } from "@/lib/api";
import type { HotkeyBinding, RetroArchInput } from "@/lib/types";

export type RetroArchField =
  | "retroarchPath"
  | "retroarchCore"
  | "retroarchPort"
  | "retroarchCommandPort"
  | "retroarchNickname"
  | "retroarchMaxPingMs";

export type RetroArchToggle =
  | "retroarchMuteSpectators"
  | "retroarchIsolatedConfig"
  | "retroarchInputEnabled";

const INPUT_FIELDS: { key: keyof RetroArchInput; label: string }[] = [
  { key: "up", label: "Up" },
  { key: "down", label: "Down" },
  { key: "left", label: "Left" },
  { key: "right", label: "Right" },
  { key: "lightPunch", label: "Light Punch" },
  { key: "mediumPunch", label: "Medium Punch" },
  { key: "heavyPunch", label: "Heavy Punch" },
  { key: "lightKick", label: "Light Kick" },
  { key: "mediumKick", label: "Medium Kick" },
  { key: "heavyKick", label: "Heavy Kick" },
  { key: "start", label: "Start" },
  { key: "coin", label: "Coin" },
];

const KEY_ALIASES: Record<string, string> = {
  ArrowUp: "up",
  ArrowDown: "down",
  ArrowLeft: "left",
  ArrowRight: "right",
  " ": "space",
  Enter: "enter",
  Escape: "escape",
  Tab: "tab",
  Backspace: "backspace",
  Shift: "shift",
  Control: "ctrl",
  Alt: "alt",
  Home: "home",
  End: "end",
  PageUp: "pageup",
  PageDown: "pagedown",
  Insert: "insert",
  Delete: "del",
  ",": "comma",
  ".": "period",
  "/": "slash",
  ";": "semicolon",
  "'": "quote",
  "[": "leftbracket",
  "]": "rightbracket",
  "\\": "backslash",
  "-": "minus",
  "=": "equals",
  "`": "backquote",
};

const KEY_LABELS: Record<string, string> = {
  up: "↑",
  down: "↓",
  left: "←",
  right: "→",
  space: "Space",
  enter: "Enter",
  escape: "Esc",
  tab: "Tab",
  backspace: "Backspace",
  shift: "Shift",
  ctrl: "Ctrl",
  alt: "Alt",
  pageup: "Page Up",
  pagedown: "Page Down",
  insert: "Insert",
  del: "Delete",
  comma: ",",
  period: ".",
  slash: "/",
  semicolon: ";",
  quote: "'",
  leftbracket: "[",
  rightbracket: "]",
  backslash: "\\",
  minus: "-",
  equals: "=",
  backquote: "`",
};

function canonicalKey(event: KeyboardEvent): string | null {
  const key = event.key;
  if (key in KEY_ALIASES) return KEY_ALIASES[key];
  if (/^[a-zA-Z]$/.test(key)) return key.toLowerCase();
  if (/^[0-9]$/.test(key)) return `num${key}`;
  if (/^F([1-9]|1[0-2])$/.test(key)) return key.toLowerCase();
  return null;
}

function keyLabel(key: string): string {
  if (key in KEY_LABELS) return KEY_LABELS[key];
  if (/^[a-z]$/.test(key)) return key.toUpperCase();
  return key;
}

interface RetroArchSettingsProps {
  path: string;
  core: string;
  port: string;
  commandPort: string;
  nickname: string;
  handle: string;
  maxPingMs: string;
  muteSpectators: boolean;
  isolatedConfig: boolean;
  input: RetroArchInput;
  inputEnabled: boolean;
  onChange: (key: RetroArchField, value: string) => void;
  onToggle: (key: RetroArchToggle, value: boolean) => void;
  onInputChange: (key: keyof RetroArchInput, value: string) => void;
  onInputReplace: (input: RetroArchInput) => void;
  onCoreDownloaded: (path: string) => void;
}

export function RetroArchSettings({
  path,
  core,
  port,
  commandPort,
  nickname,
  handle,
  maxPingMs,
  muteSpectators,
  isolatedConfig,
  input,
  inputEnabled,
  onChange,
  onToggle,
  onInputChange,
  onInputReplace,
  onCoreDownloaded,
}: RetroArchSettingsProps) {
  const [downloading, setDownloading] = useState(false);
  const [downloadError, setDownloadError] = useState<string | null>(null);
  const [hotkeys, setHotkeys] = useState<HotkeyBinding[]>([]);
  const [defaults, setDefaults] = useState<RetroArchInput | null>(null);
  const [captureField, setCaptureField] = useState<keyof RetroArchInput | null>(
    null,
  );

  useEffect(() => {
    let active = true;
    getRetroArchHotkeys()
      .then((map) => {
        if (!active) return;
        setHotkeys(map.bindings);
        setDefaults(map.defaults);
      })
      .catch(() => {});
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    if (!captureField) return;
    const handler = (event: KeyboardEvent) => {
      event.preventDefault();
      event.stopPropagation();
      if (event.key !== "Escape") {
        const canonical = canonicalKey(event);
        if (canonical) onInputChange(captureField, canonical);
      }
      setCaptureField(null);
    };
    window.addEventListener("keydown", handler, true);
    return () => window.removeEventListener("keydown", handler, true);
  }, [captureField, onInputChange]);

  async function handleDownload() {
    setDownloading(true);
    setDownloadError(null);
    try {
      const core = await downloadRetroArchCore();
      onCoreDownloaded(core.path);
    } catch (error) {
      setDownloadError(String(error));
    } finally {
      setDownloading(false);
    }
  }

  const collisionFor = (value: string) =>
    hotkeys.find(
      (hotkey) =>
        hotkey.collides &&
        hotkey.key.toLowerCase() === value.trim().toLowerCase(),
    );

  return (
    <>
      <div className="grid gap-2">
        <Label htmlFor="retroarchPath">RetroArch binary</Label>
        <Input
          id="retroarchPath"
          value={path}
          placeholder="/Applications/RetroArch.app/Contents/MacOS/RetroArch"
          onChange={(event) => onChange("retroarchPath", event.target.value)}
        />
      </div>
      <div className="grid gap-2">
        <Label htmlFor="retroarchCore">FBNeo core (frozen)</Label>
        <div className="flex gap-2">
          <Input
            id="retroarchCore"
            value={core}
            placeholder="/Users/you/Library/Application Support/RetroArch/cores/fbneo_libretro.dylib"
            onChange={(event) => onChange("retroarchCore", event.target.value)}
          />
          <Button
            type="button"
            variant="outline"
            onClick={handleDownload}
            disabled={downloading}
          >
            {downloading ? "Downloading…" : "Download frozen core"}
          </Button>
        </div>
        {downloadError ? (
          <p className="text-xs text-destructive">{downloadError}</p>
        ) : (
          <p className="text-xs text-muted-foreground">
            Leave blank to auto-detect the standard RetroArch core folder, or
            download the frozen build into the app data directory. Netplay
            refuses to sync if every machine's core revision does not match.
          </p>
        )}
      </div>
      <div className="grid gap-4 sm:grid-cols-2">
        <div className="grid gap-2">
          <Label htmlFor="retroarchPort">Netplay port</Label>
          <Input
            id="retroarchPort"
            type="number"
            min={1}
            max={65535}
            value={port}
            onChange={(event) => onChange("retroarchPort", event.target.value)}
          />
        </div>
        <div className="grid gap-2">
          <Label htmlFor="retroarchCommandPort">Command port</Label>
          <Input
            id="retroarchCommandPort"
            type="number"
            min={1}
            max={65535}
            value={commandPort}
            onChange={(event) =>
              onChange("retroarchCommandPort", event.target.value)
            }
          />
          <p className="text-xs text-muted-foreground">
            RetroArch's command-socket port on this machine. Each instance uses
            this base plus its role offset (host, client, spectator), so the
            three ports must be free.
          </p>
        </div>
        <div className="grid gap-2">
          <Label htmlFor="retroarchNickname">Netplay nickname</Label>
          <Input
            id="retroarchNickname"
            value={nickname}
            placeholder={handle || "default nickname"}
            onChange={(event) =>
              onChange("retroarchNickname", event.target.value)
            }
          />
        </div>
      </div>
      <div className="grid gap-2">
        <Label htmlFor="retroarchMaxPingMs">Max ping (ms)</Label>
        <Input
          id="retroarchMaxPingMs"
          type="number"
          min={0}
          value={maxPingMs}
          onChange={(event) =>
            onChange("retroarchMaxPingMs", event.target.value)
          }
        />
        <p className="text-xs text-muted-foreground">
          0 disables the cap. Set a ceiling so netplay drops a connection that
          drifts too far behind instead of desyncing.
        </p>
      </div>
      <div className="flex items-center justify-between gap-4">
        <div className="grid gap-1">
          <Label htmlFor="retroarchMuteSpectators">Mute spectators</Label>
          <p className="text-xs text-muted-foreground">
            Launch spectator instances with audio muted so a second copy of the
            game does not play over the match.
          </p>
        </div>
        <input
          id="retroarchMuteSpectators"
          type="checkbox"
          className="size-4 shrink-0 accent-primary"
          checked={muteSpectators}
          onChange={(event) =>
            onToggle("retroarchMuteSpectators", event.target.checked)
          }
        />
      </div>
      <div className="flex items-center justify-between gap-4">
        <div className="grid gap-1">
          <Label htmlFor="retroarchIsolatedConfig">Isolate session config</Label>
          <p className="text-xs text-muted-foreground">
            Start netplay from a minimal config instead of your RetroArch
            profile, so shaders and playlists do not leak into a match.
            Controller autoconfigs still load.
          </p>
        </div>
        <input
          id="retroarchIsolatedConfig"
          type="checkbox"
          className="size-4 shrink-0 accent-primary"
          checked={isolatedConfig}
          onChange={(event) =>
            onToggle("retroarchIsolatedConfig", event.target.checked)
          }
        />
      </div>
      <div className="grid gap-3 rounded-lg border p-3">
        <div className="flex items-start justify-between gap-4">
          <div className="grid gap-1">
            <Label>Keyboard preset (FBNeo Classic)</Label>
            <p className="text-xs text-muted-foreground">
              A 6-button fighting-game layout for Cabinet launches. Each action
              takes one key; the host binds P1 and the client binds P2.
              RetroArch hotkeys that share a preset key are disabled for the
              session.
            </p>
          </div>
          <Button
            type="button"
            variant="outline"
            size="sm"
            disabled={!inputEnabled || !defaults}
            onClick={() => defaults && onInputReplace(defaults)}
          >
            Reset
          </Button>
        </div>
        <div className="flex items-center justify-between gap-4">
          <Label htmlFor="retroarchInputEnabled" className="text-xs">
            Enable preset
          </Label>
          <input
            id="retroarchInputEnabled"
            type="checkbox"
            className="size-4 shrink-0 accent-primary"
            checked={inputEnabled}
            onChange={(event) =>
              onToggle("retroarchInputEnabled", event.target.checked)
            }
          />
        </div>
        <div
          className={`grid grid-cols-2 gap-2 sm:grid-cols-3 ${
            inputEnabled ? "" : "opacity-50"
          }`}
        >
          {INPUT_FIELDS.map(({ key, label }) => {
            const value = input[key];
            const collision = collisionFor(value);
            const capturing = captureField === key;
            return (
              <div key={key} className="grid gap-1">
                <Label className="text-xs text-muted-foreground">{label}</Label>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  disabled={!inputEnabled}
                  onClick={() => setCaptureField(capturing ? null : key)}
                  className="justify-start font-mono"
                >
                  {capturing ? "Press a key…" : keyLabel(value)}
                </Button>
                {collision ? (
                  <p className="text-[10px] leading-tight text-amber-500">
                    also {collision.action} — disabled for matches
                  </p>
                ) : null}
              </div>
            );
          })}
        </div>
      </div>
    </>
  );
}
