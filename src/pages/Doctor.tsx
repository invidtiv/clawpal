import { useEffect, useReducer, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useApi } from "@/lib/use-api";
import { initialState, reducer } from "@/lib/state";
import type { DoctorReport } from "@/lib/types";
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
import { SessionAnalysisPanel } from "@/components/SessionAnalysisPanel";

export function Doctor() {
  const { t } = useTranslation();
  const ua = useApi();
  const [state, dispatch] = useReducer(reducer, initialState);
  const [rawOutput, setRawOutput] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [lastRefreshed, setLastRefreshed] = useState<string | null>(null);

  // Logs state
  const [logsOpen, setLogsOpen] = useState(false);
  const [logsSource, setLogsSource] = useState<"clawpal" | "gateway">("clawpal");
  const [logsTab, setLogsTab] = useState<"app" | "error">("app");
  const [logsContent, setLogsContent] = useState("");
  const [logsLoading, setLogsLoading] = useState(false);
  const logsContentRef = useRef<HTMLPreElement>(null);

  const hasReport = Boolean(state.doctor);
  const autoFixable = hasReport
    ? state.doctor!.issues
        .filter((issue) => issue.autoFixable)
        .map((issue) => issue.id)
    : [];

  function runDoctorCmd(): Promise<DoctorReport> {
    return ua.runDoctor().then((report) => {
      const raw = (report as any).rawOutput;
      if (raw && typeof raw === "string") setRawOutput(raw);
      else setRawOutput(null);
      return report;
    });
  }

  function fixIssuesCmd(ids: string[]) {
    return ua.fixIssues(ids);
  }

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

  useEffect(() => {
    // Reset state when switching instances
    dispatch({ type: "setDoctor", doctor: { ok: true, score: 0, issues: [] } as any });
    dispatch({ type: "setMessage", message: "" });
    setRawOutput(null);
    if (!ua.isRemote || ua.isConnected) {
      runDoctorCmd()
        .then((report) => dispatch({ type: "setDoctor", doctor: report }))
        .catch(() =>
          dispatch({ type: "setMessage", message: t('doctor.failedRunDoctor') }),
        );
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ua.instanceId, ua.isRemote, ua.isConnected]);

  return (
    <section>
      <h2 className="text-2xl font-bold mb-4">{t('doctor.title')}</h2>

      <div className="grid grid-cols-2 gap-3 mb-6">
        {/* Health Card */}
        <Card>
            <CardHeader>
              <CardTitle>{t("doctor.health")}</CardTitle>
            </CardHeader>
            <CardContent>
              {state.doctor && (
                <>
                  <p className="text-sm text-muted-foreground mb-3">
                    {t('doctor.healthScore', { score: state.doctor.score })}
                  </p>
                  <div className="space-y-2">
                    {state.doctor.issues.map((issue) => (
                      <div
                        key={issue.id}
                        className="flex items-center gap-2 text-sm"
                      >
                        {issue.severity === "error" && (
                          <Badge variant="destructive">ERROR</Badge>
                        )}
                        {issue.severity === "warn" && (
                          <Badge variant="secondary">WARN</Badge>
                        )}
                        {issue.severity === "info" && (
                          <Badge variant="outline">INFO</Badge>
                        )}
                        <span>{issue.message}</span>
                        {issue.autoFixable && (
                          <Button
                            size="sm"
                            onClick={() => {
                              fixIssuesCmd([issue.id])
                                .then(() => runDoctorCmd())
                                .then((report) =>
                                  dispatch({ type: "setDoctor", doctor: report }),
                                )
                                .catch(() =>
                                  dispatch({
                                    type: "setMessage",
                                    message: t('doctor.failedFix'),
                                  }),
                                );
                            }}
                          >
                            {t('doctor.fix')}
                          </Button>
                        )}
                      </div>
                    ))}
                  </div>
                  <div className="flex gap-2 mt-3">
                    <Button
                      variant="outline"
                      onClick={() => {
                        fixIssuesCmd(autoFixable)
                          .then(() => runDoctorCmd())
                          .then((report) =>
                            dispatch({ type: "setDoctor", doctor: report }),
                          )
                          .catch(() =>
                            dispatch({
                              type: "setMessage",
                              message: t('doctor.failedFixAll'),
                            }),
                          );
                      }}
                      disabled={!autoFixable.length}
                    >
                      {t('doctor.fixAll')}
                    </Button>
                    <Button
                      variant="outline"
                      disabled={refreshing}
                      onClick={() => {
                        setRefreshing(true);
                        runDoctorCmd()
                          .then((report) => {
                            dispatch({ type: "setDoctor", doctor: report });
                            setLastRefreshed(new Date().toLocaleTimeString());
                          })
                          .catch(() =>
                            dispatch({
                              type: "setMessage",
                              message: t('doctor.refreshFailed'),
                            }),
                          )
                          .finally(() => setRefreshing(false));
                      }}
                    >
                      {refreshing ? t('doctor.refreshing') : t('doctor.refresh')}
                    </Button>
                    {lastRefreshed && (
                      <span className="text-xs text-muted-foreground ml-2">
                        {t('doctor.lastRefreshed', { time: lastRefreshed })}
                      </span>
                    )}
                  </div>
                </>
              )}
              {!hasReport && (
                <Button
                  onClick={() =>
                    runDoctorCmd()
                      .then((report) =>
                        dispatch({ type: "setDoctor", doctor: report }),
                      )
                  }
                >
                  {t('doctor.runDoctor')}
                </Button>
              )}
              {state.message && <p className="text-sm text-muted-foreground mt-2">{state.message}</p>}
            </CardContent>
          </Card>

        {/* Logs Card */}
        <Card>
          <CardHeader>
            <CardTitle>{t("doctor.logs")}</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-sm text-muted-foreground mb-3">{t("doctor.logsDescription")}</p>
            <div className="flex gap-2">
              <Button variant="outline" size="sm" onClick={() => openLogs("clawpal")}>
                {t("doctor.clawpalLogs")}
              </Button>
              <Button variant="outline" size="sm" onClick={() => openLogs("gateway")}>
                {t("doctor.gatewayLogs")}
              </Button>
            </div>
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
          </CardContent>
        </Card>
      </div>

      <SessionAnalysisPanel />
    </section>
  );
}
