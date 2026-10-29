import { useState } from "react";
import { ApprovalCard } from "./beautiful-ui/ApprovalCard";
import { LoadingState } from "./beautiful-ui/LoadingState";
import { TaskRow } from "./beautiful-ui/TaskRows";
import {
  fileName,
  sourceDomain,
  relativeTime,
  type Snapshot,
  type CatchJob,
} from "../lib/native";
import fr from "../locales/fr.json";
import type { Language } from "../lib/types";
type Copy = typeof fr;
function name(job: CatchJob) {
  const names = [
    ...new Set(job.files.flatMap((f) => f.faces.map((face) => face.family))),
  ];
  return names.length ? names.join(" + ") : fileName(job.sourcePath);
}
export function message(code: string, t: Copy) {
  return (t as Record<string, string>)[code] ?? code;
}
export function RecentCatches({
  snapshot,
  t,
  language,
  busy,
  onInstall,
  onIgnore,
}: {
  snapshot: Snapshot;
  t: Copy;
  language: Language;
  busy: string | null;
  onInstall: (job: CatchJob) => void;
  onIgnore: (id: string) => void;
}) {
  const [historyLimit, setHistoryLimit] = useState(50);
  const pending = snapshot.recent.filter((j) => j.status === "pending");
  const history = snapshot.recent.filter((j) => j.status !== "pending");
  const watcherStarting = snapshot.watcherErrors.includes("watcher_starting");
  const watcherErrors = snapshot.watcherErrors.filter(
    (error) => error !== "watcher_starting",
  );
  return (
    <>
      <div className="section-title">
        <div>
          <h1>{t.recentTitle}</h1>
        </div>
        {pending.length > 0 && (
          <span>
            {pending.length} {t.toApprove}
          </span>
        )}
      </div>
      {watcherStarting && (
        <div className="watcher-warning" role="status">
          {t.watcherStarting}
        </div>
      )}
      {watcherErrors.length > 0 && (
        <div className="watcher-warning" role="status">
          {t.watchWarning}
          {watcherErrors.map((e) => (
            <p key={e}>
              {e.startsWith("folder_unavailable:")
                ? e.slice("folder_unavailable:".length)
                : e}
            </p>
          ))}
        </div>
      )}
      {snapshot.processing && (
        <LoadingState label={`${t.extracting} ${snapshot.processing}`} />
      )}
      <div className="pending-list">
        {pending.map((job) => {
          const faces = job.files.flatMap((f) => f.faces);
          return (
            <ApprovalCard
              key={job.id}
              title={job.requiresUpdate ? t.updateAvailable : t.newFont}
              installLabel={job.requiresUpdate ? t.updateFont : t.installReal}
              ignoreLabel={t.ignore}
              disabled={Boolean(busy)}
              onInstall={() => onInstall(job)}
              onIgnore={() => onIgnore(job.id)}
            >
              <div className="approval-font">
                <span className="approval-aa">Ag</span>
                <div>
                  <h2>{name(job)}</h2>
                  <p>
                    {faces.length} {faces.length === 1 ? t.style : t.styles} ·{" "}
                    {faces.every((f) => f.variable)
                      ? t.variable
                      : faces.every((f) => !f.variable)
                        ? t.static
                        : t.both}
                  </p>
                  <span>
                    {sourceDomain(job.sourceUrl) || fileName(job.sourcePath)}
                  </span>
                </div>
              </div>
              {job.requiresUpdate && (
                <p className="catch-warning">{t.updateExplanation}</p>
              )}
              {job.warnings.map((w) => (
                <p className="catch-warning" key={w}>
                  {message(w, t)}
                </p>
              ))}
            </ApprovalCard>
          );
        })}
      </div>
      {history.length > 0 && (
        <>
          <div className="list-label">
            {t.history}
            <span>{history.length}</span>
          </div>
          <div className="task-list">
            {history.slice(0, historyLimit).map((job, index) => (
              <TaskRow
                key={job.id}
                name={name(job)}
                meta={`${sourceDomain(job.sourceUrl) || fileName(job.sourcePath)} · ${relativeTime(job.createdAt, language)}`}
                status={t[job.status]}
                installed={job.status === "installed"}
                index={Math.min(index, 4)}
              >
                {job.status === "installing" && (
                  <LoadingState label={t.installing} />
                )}
                {job.status === "uninstalling" && (
                  <LoadingState label={t.uninstalling} />
                )}
                <p>{fileName(job.sourcePath)}</p>
                {job.files
                  .flatMap((f) => f.faces)
                  .map((f) => (
                    <p key={f.postscript}>
                      {f.family} · {f.style} · {f.version}
                    </p>
                  ))}
                {job.status === "installed" && (
                  <p>
                    {job.installedCount}{" "}
                    {job.installedCount === 1 ? t.style : t.styles} ·{" "}
                    {t.installed}
                  </p>
                )}
                {(job.status === "uninstalled" ||
                  job.status === "uninstall_failed") && (
                  <p>
                    {job.removedCount} {t.removedStyles}
                  </p>
                )}
                {job.error && (
                  <p className="catch-error">{message(job.error, t)}</p>
                )}
                {job.warnings.map((w) => (
                  <p className="catch-warning" key={w}>
                    {message(w, t)}
                  </p>
                ))}
                {job.status === "failed" && job.files.length > 0 && (
                  <div className="catch-actions">
                    <button
                      className="secondary-button"
                      disabled={Boolean(busy) || job.planned.length > 0}
                      onClick={() => onInstall(job)}
                    >
                      {t.retry}
                    </button>
                    <button
                      className="quiet-button"
                      disabled={Boolean(busy) || job.planned.length > 0}
                      onClick={() => onIgnore(job.id)}
                    >
                      {t.ignore}
                    </button>
                  </div>
                )}
              </TaskRow>
            ))}
          </div>
          {history.length > historyLimit && (
            <button
              className="secondary-button show-more"
              onClick={() => setHistoryLimit((n) => n + 50)}
            >
              {t.showMore}
            </button>
          )}
        </>
      )}
    </>
  );
}
