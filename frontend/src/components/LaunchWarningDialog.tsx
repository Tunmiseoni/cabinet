import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

interface LaunchWarningDialogProps {
  warnings: string[] | null;
  onCancel: () => void;
  onConfirm: () => void;
}

export function LaunchWarningDialog({
  warnings,
  onCancel,
  onConfirm,
}: LaunchWarningDialogProps) {
  return (
    <Dialog open={warnings !== null} onOpenChange={(open) => !open && onCancel()}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Connection warning</DialogTitle>
          <DialogDescription>
            The selected peer may be unreachable, or may not give a good match.
            Launch anyway?
          </DialogDescription>
        </DialogHeader>
        <ul className="list-disc space-y-1 pl-5 text-sm">
          {(warnings ?? []).map((warning) => (
            <li key={warning}>{warning}</li>
          ))}
        </ul>
        <DialogFooter>
          <Button variant="outline" onClick={onCancel}>
            Cancel
          </Button>
          <Button onClick={onConfirm}>Launch anyway</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
