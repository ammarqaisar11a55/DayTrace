import { useEffect, useState } from "react";
import { api } from "../services/api";

const cache = new Map<number, string | null>();

/** Application ids are reused once applications are deleted. */
export function clearIconCache() {
  cache.clear();
}

/** The application's own icon, or its initial when the platform has none. */
export function AppIcon({ id, name, size = 20 }: { id: number | null; name: string; size?: number }) {
  const [source, setSource] = useState<string | null>(id === null ? null : (cache.get(id) ?? null));

  useEffect(() => {
    if (id === null) return;
    if (cache.has(id)) {
      setSource(cache.get(id) ?? null);
      return;
    }
    let current = true;
    api.applicationIcon(id).then(
      (icon) => {
        cache.set(id, icon);
        if (current) setSource(icon);
      },
      () => cache.set(id, null),
    );
    return () => {
      current = false;
    };
  }, [id]);

  if (source) return <img src={source} alt="" width={size} height={size} className="shrink-0 object-contain" />;
  return (
    <span
      aria-hidden
      className="inline-flex shrink-0 items-center justify-center rounded bg-rule font-semibold text-ink-2"
      style={{ width: size, height: size, fontSize: size * 0.5 }}
    >
      {name.charAt(0).toUpperCase()}
    </span>
  );
}
