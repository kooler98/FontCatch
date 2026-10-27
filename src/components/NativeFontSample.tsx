import { useEffect, useRef, useState } from "react";
import { useFontPreview } from "../lib/font-preview";
import type { InstalledFont } from "../lib/native";
export function NativeFontSample({
  font,
  loading,
  unavailable,
}: {
  font?: InstalledFont;
  loading: string;
  unavailable: string;
}) {
  const target = useRef<HTMLSpanElement>(null);
  const [visible, setVisible] = useState(false);
  useEffect(() => {
    const observer = new IntersectionObserver(
      ([entry]) => setVisible(entry.isIntersecting),
      { rootMargin: "80px" },
    );
    if (target.current) observer.observe(target.current);
    return () => observer.disconnect();
  }, []);
  const preview = useFontPreview(font, visible);
  return (
    <span
      ref={target}
      className={`type-sample real-sample ${preview.status}`}
      style={
        preview.status === "ready"
          ? { fontFamily: `"${preview.family}"`, fontSynthesis: "none" }
          : undefined
      }
    >
      {preview.status === "ready" ? (
        "Aa Bb Cc Dd"
      ) : (
        <small>{preview.status === "error" ? unavailable : loading}</small>
      )}
    </span>
  );
}
