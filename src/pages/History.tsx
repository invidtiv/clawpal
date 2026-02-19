import { useEffect, useState } from "react";
import { api } from "../lib/api";
import { useInstance } from "@/lib/instance-context";
import { DiffViewer } from "../components/DiffViewer";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { HistoryItem, PreviewResult } from "../lib/types";
import { formatTime } from "@/lib/utils";

export function History() {
  const { instanceId, isRemote, isConnected } = useInstance();
  const [history, setHistory] = useState<HistoryItem[]>([]);
  const [preview, setPreview] = useState<PreviewResult | null>(null);
  const [message, setMessage] = useState("");

  const refreshHistory = () => {
    if (isRemote) {
      if (!isConnected) return;
      return api.remoteListHistory(instanceId)
        .then((resp) => setHistory(resp.items))
        .catch(() => setMessage("Failed to load history"));
    }
    return api.listHistory(50, 0)
      .then((resp) => setHistory(resp.items))
      .catch(() => setMessage("Failed to load history"));
  };

  useEffect(() => {
    refreshHistory();
  }, [instanceId, isRemote, isConnected]);

  // Build a map from snapshot ID to its display info for rollback references
  const historyMap = new Map(
    history.map((h) => [h.id, h])
  );

  return (
    <section>
      <h2 className="text-2xl font-bold mb-4">History</h2>
      <div className="space-y-3">
        {history.map((item) => {
          const isRollback = item.source === "rollback";
          const rollbackTarget = item.rollbackOf ? historyMap.get(item.rollbackOf) : undefined;
          return (
            <Card key={item.id} className={isRollback ? "border-dashed opacity-75" : ""}>
              <CardContent>
                <div className="flex items-center gap-2 text-sm flex-wrap">
                  <span className="text-muted-foreground">{formatTime(item.createdAt)}</span>
                  {isRollback ? (
                    <>
                      <Badge variant="outline">rollback</Badge>
                      <span className="text-muted-foreground">
                        Reverted {rollbackTarget
                          ? `"${rollbackTarget.recipeId || "manual"}" from ${formatTime(rollbackTarget.createdAt)}`
                          : item.recipeId || "unknown"
                        }
                      </span>
                    </>
                  ) : (
                    <>
                      <Badge variant="secondary">{item.recipeId || "manual"}</Badge>
                      <span className="text-muted-foreground">{item.source}</span>
                    </>
                  )}
                  {!item.canRollback && !isRollback && (
                    <Badge variant="outline" className="text-muted-foreground">not rollbackable</Badge>
                  )}
                </div>
                {!isRollback && (
                  <div className="flex gap-2 mt-2">
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={async () => {
                        try {
                          const p = isRemote
                            ? await api.remotePreviewRollback(instanceId, item.id)
                            : await api.previewRollback(item.id);
                          setPreview(p);
                        } catch (err) {
                          setMessage(String(err));
                        }
                      }}
                      disabled={!item.canRollback}
                    >
                      Preview
                    </Button>
                    <Button
                      variant="destructive"
                      size="sm"
                      onClick={async () => {
                        try {
                          if (isRemote) {
                            await api.remoteRollback(instanceId, item.id);
                          } else {
                            await api.rollback(item.id);
                          }
                          setMessage("Rollback completed");
                          await refreshHistory();
                        } catch (err) {
                          setMessage(String(err));
                        }
                      }}
                      disabled={!item.canRollback}
                    >
                      Rollback
                    </Button>
                  </div>
                )}
              </CardContent>
            </Card>
          );
        })}
      </div>
      <Button variant="outline" onClick={refreshHistory} className="mt-3">
        Refresh
      </Button>
      {message && (
        <p className="text-sm text-muted-foreground mt-2">{message}</p>
      )}

      {/* Preview Dialog */}
      <Dialog open={!!preview} onOpenChange={(open) => { if (!open) setPreview(null); }}>
        <DialogContent className="max-w-3xl">
          <DialogHeader>
            <DialogTitle>Rollback Preview</DialogTitle>
          </DialogHeader>
          {preview && (
            <DiffViewer
              oldValue={preview.configBefore}
              newValue={preview.configAfter}
            />
          )}
        </DialogContent>
      </Dialog>
    </section>
  );
}
