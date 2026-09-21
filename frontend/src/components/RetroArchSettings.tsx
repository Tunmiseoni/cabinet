import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { downloadRetroArchCore } from "@/lib/api";

export type RetroArchField =
  | "retroarchPath"
  | "retroarchCore"
  | "retroarchPort"
  | "retroarchNickname"
  | "retroarchMaxPingMs";

export type RetroArchToggle =
  | "retroarchMuteSpectators"
  | "retroarchIsolatedConfig";

interface RetroArchSettingsProps {
  path: string;
  core: string;
  port: string;
  nickname: string;
  handle: string;
  maxPingMs: string;
  muteSpectators: boolean;
  isolatedConfig: boolean;
  onChange: (key: RetroArchField, value: string) => void;
  onToggle: (key: RetroArchToggle, value: boolean) => void;
  onCoreDownloaded: (path: string) => void;
}

export function RetroArchSettings({
  path,
  core,
  port,
  nickname,
  handle,
  maxPingMs,
  muteSpectators,
  isolatedConfig,
  onChange,
  onToggle,
  onCoreDownloaded,
}: RetroArchSettingsProps) {
  const [downloading, setDownloading] = useState(false);
  const [downloadError, setDownloadError] = useState<string | null>(null);

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
          <Label htmlFor="retroarchNickname">Netplay nickname</Label>
          <Input
            id="retroarchNickname"
            value={nickname}
            placeholder={handle || "default nickname"}
            onChange={(event) => onChange("retroarchNickname", event.target.value)}
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
          onChange={(event) => onChange("retroarchMaxPingMs", event.target.value)}
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
            Start netplay from a minimal config instead of your RetroArch profile,
            so shaders and playlists do not leak into a match. Controller
            autoconfigs still load.
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
    </>
  );
}
