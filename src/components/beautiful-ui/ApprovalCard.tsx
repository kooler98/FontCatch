// Adapted from Beautiful UI Approval Card, © 2026 Shane Levine. MIT — see ./LICENSE.
// The single font decision uses its card/footer/pill-action structure, without the multi-question wizard.
import type { ReactNode } from "react";
export function ApprovalCard({
  title,
  children,
  installLabel,
  ignoreLabel,
  onInstall,
  onIgnore,
  disabled = false,
}: {
  title: string;
  children: ReactNode;
  installLabel: string;
  ignoreLabel: string;
  onInstall: () => void;
  onIgnore: () => void;
  disabled?: boolean;
}) {
  return (
    <article
      className="approval-card"
      style={{ animation: "pop-in 360ms cubic-bezier(0.22,1,0.36,1) both" }}
    >
      <div className="approval-content">
        <div className="approval-eyebrow">
          <span className="pending-dot" />
          {title}
        </div>
        {children}
      </div>
      <div className="approval-footer">
        <button className="quiet-button" disabled={disabled} onClick={onIgnore}>
          {ignoreLabel}
        </button>
        <button
          className="primary-button"
          disabled={disabled}
          onClick={onInstall}
        >
          {installLabel}
          <span aria-hidden>↗</span>
        </button>
      </div>
    </article>
  );
}
