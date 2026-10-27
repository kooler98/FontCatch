import type { CSSProperties } from "react";
const paths = {
  recent: "M3 11a9 9 0 1 1 2.5 7M3 4v7h7M12 7v5l3 2",
  library: "M4 4h4v16H4zM11 4h4v16h-4zM18 4l4 15-3 1-4-15z",
  settings: "M4 7h16M4 17h16M8 4v6M16 14v6",
  folder: "M3 7h7l2-3h8a1 1 0 0 1 1 1v14H3z",
  plus: "M12 5v14M5 12h14",
  arrow: "M5 12h14M13 6l6 6-6 6",
  back: "M19 12H5M11 6l-6 6 6 6",
  close: "m6 6 12 12M18 6 6 18",
  coffee:
    "M4 8h13v7a5 5 0 0 1-5 5H9a5 5 0 0 1-5-5zM17 9h2a3 3 0 0 1 0 6h-2M7 3v2M12 3v2",
  lock: "M7 11V7a5 5 0 0 1 10 0v4M5 11h14v10H5z",
  copy: "M9 9h12v12H9zM15 9V3H3v12h6",
  chevron: "m9 5 7 7-7 7",
  trash: "M3 6h18M9 6V3h6v3M5 6l1 15h12l1-15M10 10v7M14 10v7",
};
export function Icon({
  name,
  size = 16,
  style,
}: {
  name: keyof typeof paths;
  size?: number;
  style?: CSSProperties;
}) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
      style={style}
    >
      <path d={paths[name]} />
    </svg>
  );
}
