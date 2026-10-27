// Adapted from Beautiful UI Filter Table chips, © 2026 Shane Levine. MIT — see ./LICENSE.
export function FilterChips<T extends string>({
  value,
  options,
  onChange,
  label,
}: {
  value: T;
  options: { value: T; label: string; count?: number }[];
  onChange: (value: T) => void;
  label: string;
}) {
  return (
    <div
      className="filter-chips flex items-center gap-1 overflow-x-auto py-1"
      role="group"
      aria-label={label}
    >
      {options.map((option) => {
        const active = option.value === value;
        return (
          <button
            key={option.value}
            type="button"
            aria-pressed={active}
            onClick={() => onChange(option.value)}
            className={`filter-chip flex h-7 shrink-0 items-center gap-1.5 rounded-full px-2.5 text-[11px] font-medium transition-[background-color,box-shadow,color] duration-200 ${active ? "is-active" : ""}`}
          >
            {option.label}
            {option.count !== undefined && (
              <span className="chip-count rounded px-1 text-[10px] tabular-nums">
                {option.count}
              </span>
            )}
          </button>
        );
      })}
    </div>
  );
}
