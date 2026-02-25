import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useApi } from "@/lib/use-api";
import { useInstance } from "@/lib/instance-context";
import { useDoctorAgent } from "@/lib/use-doctor-agent";
import {
  Card,
  CardHeader,
  CardTitle,
  CardContent,
} from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { DoctorChat } from "@/components/DoctorChat";

export function Doctor() {
  const { t } = useTranslation();
  const ua = useApi();
  const { instanceId, isDocker, isRemote } = useInstance();
  const doctor = useDoctorAgent();

  const [diagnosing, setDiagnosing] = useState(false);
  const [startError, setStartError] = useState<string | null>(null);

  // Full-auto confirmation dialog
  const [fullAutoConfirmOpen, setFullAutoConfirmOpen] = useState(false);

  // Logs state
  const [logsOpen, setLogsOpen] = useState(false);
  const [logsSource, setLogsSource] = useState<"clawpal" | "gateway">("clawpal");
  const [logsTab, setLogsTab] = useState<"app" | "error">("app");
  const [logsContent, setLogsContent] = useState("");
  const [logsLoading, setLogsLoading] = useState(false);
  const logsContentRef = useRef<HTMLPreElement>(null);

  // Keep execution target synced with current instance tab:
  // - local/docker: execute on local machine
  // - remote ssh: execute on selected remote host
  useEffect(() => {
    doctor.setTarget(isRemote ? instanceId : "local");
  }, [doctor.setTarget, instanceId, isRemote]);

  const handleStartDiagnosis = async () => {
    setStartError(null);
    setDiagnosing(true);
    try {
      const diagnosisScope = isRemote
        ? instanceId
        : isDocker
          ? instanceId
          : "local";
      const executionTarget = isRemote ? instanceId : "local";
      doctor.setTarget(executionTarget);

      if (isRemote) {
        const status = await ua.sshStatus(instanceId);
        if (status !== "connected") {
          await ua.sshConnect(instanceId);
        }
      }

      await doctor.connect();
      const context = isRemote
        ? await ua.collectDoctorContextRemote(instanceId)
        : await ua.collectDoctorContext();
      const diagnosisTransport: "local" | "docker_local" | "remote_ssh" = isRemote
        ? "remote_ssh"
        : isDocker
          ? "docker_local"
          : "local";
      await doctor.startDiagnosis(context, "main", diagnosisScope, diagnosisTransport);
    } catch (err) {
      const msg = String(err);
      setStartError(msg);
    } finally {
      setDiagnosing(false);
    }
  };

  const handleStopDiagnosis = async () => {
    await doctor.disconnect();
    doctor.reset();
  };

  // Logs helpers
  const fetchLog = (source: "clawpal" | "gateway", which: "app" | "error") => {
    setLogsLoading(true);
    const fn = source === "clawpal"
      ? (which === "app" ? ua.readAppLog : ua.readErrorLog)
      : (which === "app" ? ua.readGatewayLog : ua.readGatewayErrorLog);
    fn(500)
      .then((text) => {
        setLogsContent(text);
        setTimeout(() => {
          if (logsContentRef.current) {
            logsContentRef.current.scrollTop = logsContentRef.current.scrollHeight;
          }
        }, 50);
      })
      .catch(() => setLogsContent(""))
      .finally(() => setLogsLoading(false));
  };

  const openLogs = (source: "clawpal" | "gateway") => {
    setLogsSource(source);
    setLogsTab("app");
    setLogsOpen(true);
  };

  useEffect(() => {
    if (logsOpen) fetchLog(logsSource, logsTab);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [logsOpen, logsSource, logsTab]);

  return (
    <section>
      <h2 className="text-2xl font-bold mb-4">{t("doctor.title")}</h2>

      <Card className="gap-2 py-4">
        <CardHeader className="pb-0">
          <div className="flex items-center justify-between">
            <CardTitle className="text-base">{t("doctor.agentSource")}</CardTitle>
            <div className="flex items-center gap-1">
              <Button variant="ghost" size="sm" onClick={() => openLogs("clawpal")}>
                {t("doctor.clawpalLogs")}
              </Button>
              <Button variant="ghost" size="sm" onClick={() => openLogs("gateway")}>
                {t("doctor.gatewayLogs")}
              </Button>
            </div>
          </div>
        </CardHeader>
        <CardContent>
          {!doctor.connected && doctor.messages.length === 0 ? (
            <>
              {startError && (
                <div className="mb-3 text-sm text-destructive">{startError}</div>
              )}
              {doctor.error && (
                <div className="mb-3 text-sm text-destructive">{doctor.error}</div>
              )}
              <Button onClick={handleStartDiagnosis} disabled={diagnosing}>
                {diagnosing ? t("doctor.connecting") : t("doctor.startDiagnosis")}
              </Button>
            </>
          ) : !doctor.connected && doctor.messages.length > 0 ? (
            <>
              {/* Disconnected mid-session — show chat with reconnect banner */}
              <div className="flex items-center justify-between mb-3 p-2 rounded-md bg-destructive/10 border border-destructive/20">
                <span className="text-sm text-destructive">
                  {doctor.error || t("doctor.disconnected")}
                </span>
                <div className="flex items-center gap-2">
                  <Button size="sm" onClick={() => doctor.reconnect()}>
                    {t("doctor.reconnect")}
                  </Button>
                  <Button variant="outline" size="sm" onClick={handleStopDiagnosis}>
                    {t("doctor.stopDiagnosis")}
                  </Button>
                </div>
              </div>
              <DoctorChat
                messages={doctor.messages}
                loading={false}
                error={null}
                connected={false}
                onSendMessage={doctor.sendMessage}
                onApproveInvoke={doctor.approveInvoke}
                onRejectInvoke={doctor.rejectInvoke}
              />
            </>
          ) : (
            <>
              <div className="flex items-center justify-between mb-3">
                <div className="flex items-center gap-2">
                  <Badge variant="outline" className="text-xs">
                    {t("doctor.engineZeroclaw")}
                  </Badge>
                  <Badge variant="outline" className="text-xs flex items-center gap-1.5">
                    <span className={`inline-block w-1.5 h-1.5 rounded-full ${doctor.bridgeConnected ? "bg-emerald-500" : "bg-muted-foreground/40"}`} />
                    {doctor.bridgeConnected ? t("doctor.bridgeConnected") : t("doctor.bridgeDisconnected")}
                  </Badge>
                </div>
                <div className="flex items-center gap-2">
                  <label className="flex items-center gap-1.5 text-xs cursor-pointer select-none">
                    <input
                      type="checkbox"
                      checked={doctor.fullAuto}
                      onChange={(e) => {
                        if (e.target.checked) {
                          setFullAutoConfirmOpen(true);
                        } else {
                          doctor.setFullAuto(false);
                        }
                      }}
                      className="accent-primary"
                    />
                    {t("doctor.fullAuto")}
                  </label>
                  <Button variant="outline" size="sm" onClick={handleStopDiagnosis}>
                    {t("doctor.stopDiagnosis")}
                  </Button>
                </div>
              </div>
              <DoctorChat
                messages={doctor.messages}
                loading={doctor.loading}
                error={doctor.error}
                connected={doctor.connected}
                onSendMessage={doctor.sendMessage}
                onApproveInvoke={doctor.approveInvoke}
                onRejectInvoke={doctor.rejectInvoke}
              />
            </>
          )}
        </CardContent>
      </Card>

      {/* Full-Auto Confirmation */}
      <Dialog open={fullAutoConfirmOpen} onOpenChange={setFullAutoConfirmOpen}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>{t("doctor.fullAutoTitle")}</DialogTitle>
          </DialogHeader>
          <p className="text-sm text-muted-foreground">{t("doctor.fullAutoWarning")}</p>
          <div className="flex justify-end gap-2 mt-4">
            <Button variant="outline" size="sm" onClick={() => setFullAutoConfirmOpen(false)}>
              {t("doctor.cancel")}
            </Button>
            <Button variant="destructive" size="sm" onClick={() => {
              doctor.setFullAuto(true);
              setFullAutoConfirmOpen(false);
            }}>
              {t("doctor.fullAutoConfirm")}
            </Button>
          </div>
        </DialogContent>
      </Dialog>

      {/* Logs Dialog */}
      <Dialog open={logsOpen} onOpenChange={setLogsOpen}>
        <DialogContent className="sm:max-w-2xl max-h-[80vh] flex flex-col">
          <DialogHeader>
            <DialogTitle>
              {logsSource === "clawpal" ? t("doctor.clawpalLogs") : t("doctor.gatewayLogs")}
            </DialogTitle>
          </DialogHeader>
          <div className="flex items-center gap-2 mb-2">
            <Button
              variant={logsTab === "app" ? "default" : "outline"}
              size="sm"
              onClick={() => setLogsTab("app")}
            >
              {t("doctor.appLog")}
            </Button>
            <Button
              variant={logsTab === "error" ? "default" : "outline"}
              size="sm"
              onClick={() => setLogsTab("error")}
            >
              {t("doctor.errorLog")}
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => fetchLog(logsSource, logsTab)}
              disabled={logsLoading}
            >
              {t("doctor.refreshLogs")}
            </Button>
          </div>
          <pre
            ref={logsContentRef}
            className="flex-1 min-h-[300px] max-h-[60vh] overflow-auto rounded-md border bg-muted p-3 text-xs font-mono whitespace-pre-wrap break-all"
          >
            {logsContent || t("doctor.noLogs")}
          </pre>
        </DialogContent>
      </Dialog>
    </section>
  );
}
