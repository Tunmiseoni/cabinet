import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import type { RomIndex } from "@/lib/types";
import { formatBytes } from "@/lib/format";
import { Gamepad2 } from "lucide-react";

interface RomsCardProps {
  romIndex: RomIndex | null;
  loading: boolean;
  error: string | null;
}

export function RomsCard({ romIndex, loading, error }: RomsCardProps) {
  const roms = romIndex?.roms ?? [];

  return (
    <Card className="flex min-h-0 flex-col">
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Gamepad2 className="size-4" />
          ROMs
        </CardTitle>
        <CardDescription className="truncate">
          {error
            ? "Could not read ROM directory"
            : loading
              ? "Scanning ROM directory…"
              : romIndex?.dir
                ? `${roms.length} ROM${roms.length === 1 ? "" : "s"} · ${romIndex.dir}`
                : "No ROM directory found — set one in settings"}
        </CardDescription>
      </CardHeader>
      <CardContent className="min-h-0 flex-1">
        <ScrollArea className="h-64 pr-3">
          {roms.map((rom) => (
            <div key={rom.path}>
              <div className="flex items-center justify-between gap-3 py-2">
                <span className="truncate font-mono text-sm">
                  {rom.shortName}
                </span>
                <span className="shrink-0 text-xs text-muted-foreground">
                  {formatBytes(rom.sizeBytes)}
                </span>
              </div>
              <Separator />
            </div>
          ))}
          {roms.length === 0 && !loading && !error && (
            <p className="py-6 text-center text-sm text-muted-foreground">
              No ROMs found.
            </p>
          )}
        </ScrollArea>
      </CardContent>
    </Card>
  );
}
