import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { InstallMethod, InstallMethodCapability, InstallSession } from "@/lib/types";
import { useApi } from "@/lib/use-api";

const METHOD_ORDER: InstallMethod[] = ["local", "wsl2", "docker", "remote_ssh"];

function sortMethods(methods: InstallMethodCapability[]): InstallMethodCapability[] {
  const rank = new Map(METHOD_ORDER.map((method, index) => [method, index]));
  return [...methods].sort((a, b) => (rank.get(a.method) ?? 99) - (rank.get(b.method) ?? 99));
}

export function InstallHub({
  showToast,
}: {
  showToast?: (message: string, type?: "success" | "error") => void;
}) {
  const { t } = useTranslation();
  const ua = useApi();
  const [methods, setMethods] = useState<InstallMethodCapability[]>([]);
  const [loadingMethods, setLoadingMethods] = useState(true);
  const [selectedMethod, setSelectedMethod] = useState<InstallMethod>("local");
  const [creating, setCreating] = useState(false);
  const [session, setSession] = useState<InstallSession | null>(null);

  useEffect(() => {
    setLoadingMethods(true);
    ua.listInstallMethods()
      .then((result) => {
        const sorted = sortMethods(result);
        setMethods(sorted);
        if (sorted.length > 0) {
          setSelectedMethod(sorted[0].method);
        }
      })
      .catch((e) => showToast?.(String(e), "error"))
      .finally(() => setLoadingMethods(false));
  }, [ua, showToast]);

  const selectedMeta = useMemo(
    () => methods.find((m) => m.method === selectedMethod) ?? null,
    [methods, selectedMethod],
  );

  const methodLabel = (method: InstallMethod): string => t(`home.install.method.${method}`);

  const handleCreateSession = () => {
    setCreating(true);
    ua.installCreateSession(selectedMethod)
      .then((next) => {
        setSession(next);
        showToast?.(t("home.install.sessionCreated"), "success");
      })
      .catch((e) => showToast?.(String(e), "error"))
      .finally(() => setCreating(false));
  };

  return (
    <>
      <h3 className="text-lg font-semibold mt-8 mb-4">{t("home.install.title")}</h3>
      <Card>
        <CardContent className="space-y-4">
          <p className="text-sm text-muted-foreground">{t("home.install.description")}</p>
          <div className="flex flex-wrap items-center gap-2">
            <Select
              value={selectedMethod}
              onValueChange={(value) => setSelectedMethod(value as InstallMethod)}
              disabled={loadingMethods || creating}
            >
              <SelectTrigger size="sm" className="w-[240px]">
                <SelectValue placeholder={t("home.install.selectMethod")} />
              </SelectTrigger>
              <SelectContent>
                {methods.map((method) => (
                  <SelectItem key={method.method} value={method.method}>
                    {methodLabel(method.method)}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            {selectedMeta && (
              <Badge variant={selectedMeta.available ? "secondary" : "outline"}>
                {selectedMeta.available
                  ? t("home.install.available")
                  : t("home.install.needsSetup")}
              </Badge>
            )}
            <Button size="sm" disabled={creating || loadingMethods} onClick={handleCreateSession}>
              {creating ? t("home.install.creating") : t("home.install.start")}
            </Button>
          </div>
          {selectedMeta?.hint && (
            <p className="text-xs text-muted-foreground">{selectedMeta.hint}</p>
          )}
          {session && (
            <div className="rounded-md border p-3 text-sm">
              <div className="font-medium">{t("home.install.currentSession")}</div>
              <div className="text-muted-foreground">ID: {session.id}</div>
              <div className="text-muted-foreground">
                {t("home.install.sessionState", { state: session.state })}
              </div>
            </div>
          )}
        </CardContent>
      </Card>
    </>
  );
}
