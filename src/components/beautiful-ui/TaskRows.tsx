// Adapted from Beautiful UI Task Rows, © 2026 Shane Levine. MIT — see ./LICENSE.
// Status is supplied by the app; the original gallery's timed fake transitions are removed.
import { useState } from "react";
import type { ReactNode } from "react";
export function TaskRow({
  name,
  meta,
  status,
  installed,
  children,
  index = 0,
}: {
  name: string;
  meta: string;
  status: string;
  installed?: boolean;
  children: ReactNode;
  index?: number;
}) {
  const [open, setOpen] = useState(false);
  return (
    <div
      className="task-row"
      style={{
        animation: `fade-up 450ms cubic-bezier(0.23,1,0.32,1) ${index * 80}ms both`,
      }}
    >
      <button
        type="button"
        aria-expanded={open}
        onClick={() => setOpen(!open)}
        className="task-trigger"
      >
        <span
          className={`font-tile ${installed ? "installed" : ""}`}
          aria-hidden
        >
          {installed ? (
            <svg
              width="17"
              height="17"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2.4"
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              <path d="M20 6L9 17l-5-5" />
            </svg>
          ) : (
            "Aa"
          )}
        </span>
        <span className="task-copy">
          <strong>{name}</strong>
          <span>{meta}</span>
        </span>
        <span className="status-label">{status}</span>
        <svg
          width="12"
          height="12"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          style={{ transform: open ? "rotate(180deg)" : undefined }}
          aria-hidden
        >
          <path d="m6 9 6 6 6-6" />
        </svg>
      </button>
      <div
        className="grid transition-[grid-template-rows,opacity] duration-300"
        style={{
          gridTemplateRows: open ? "1fr" : "0fr",
          opacity: open ? 1 : 0,
        }}
        inert={!open}
      >
        <div className="overflow-hidden">
          {open && <div className="task-detail">{children}</div>}
        </div>
      </div>
    </div>
  );
}
