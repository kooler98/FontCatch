// Adapted from Beautiful UI Loading State, © 2026 Shane Levine. MIT — see ./LICENSE.
export function LoadingState({ label }: { label: string }) {
  return (
    <div role="status" className="flex items-center gap-2.5 py-4">
      <span className="loading-indicator" aria-hidden />
      <span className="text-[12px] font-medium text-ink">{label}</span>
    </div>
  );
}
