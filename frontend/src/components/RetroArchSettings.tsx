import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

export type RetroArchField =
  | "retroarchPath"
  | "retroarchCore"
  | "retroarchPort"
  | "retroarchNickname";

interface RetroArchSettingsProps {
  path: string;
  core: string;
  port: string;
  nickname: string;
  handle: string;
  onChange: (key: RetroArchField, value: string) => void;
}

export function RetroArchSettings({
  path,
  core,
  port,
  nickname,
  handle,
  onChange,
}: RetroArchSettingsProps) {
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
        <Input
          id="retroarchCore"
          value={core}
          placeholder="/Users/you/Library/Application Support/RetroArch/cores/fbneo_libretro.dylib"
          onChange={(event) => onChange("retroarchCore", event.target.value)}
        />
        <p className="text-xs text-muted-foreground">
          Leave blank to auto-detect the standard RetroArch core folder. Netplay
          refuses to sync if every machine's core revision does not match.
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
    </>
  );
}
