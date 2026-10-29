import { LoaderCircle, RefreshCw } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { t } from "../lib/i18n";

export function PullToRefresh({ onRefresh }: { onRefresh: () => Promise<unknown> }) {
  const start = useRef<number | undefined>(undefined);
  const distance = useRef(0);
  const [pull, setPull] = useState(0);
  const [refreshing, setRefreshing] = useState(false);
  const refresh = useRef(onRefresh);
  refresh.current = onRefresh;

  useEffect(() => {
    const begin = (event: TouchEvent) => {
      const target = event.target as HTMLElement | null;
      if (window.scrollY <= 0 && !target?.closest("input, textarea, select")) {
        start.current = event.touches[0]?.clientY;
      }
    };
    const move = (event: TouchEvent) => {
      if (start.current == null || refreshing) return;
      const delta = Math.max(0, (event.touches[0]?.clientY ?? start.current) - start.current);
      distance.current = Math.min(86, delta * 0.42);
      setPull(distance.current);
    };
    const end = () => {
      const shouldRefresh = distance.current >= 54 && !refreshing;
      start.current = undefined;
      distance.current = 0;
      setPull(0);
      if (shouldRefresh) {
        setRefreshing(true);
        void refresh.current().finally(() => setRefreshing(false));
      }
    };
    document.addEventListener("touchstart", begin, { passive: true });
    document.addEventListener("touchmove", move, { passive: true });
    document.addEventListener("touchend", end, { passive: true });
    document.addEventListener("touchcancel", end, { passive: true });
    return () => {
      document.removeEventListener("touchstart", begin);
      document.removeEventListener("touchmove", move);
      document.removeEventListener("touchend", end);
      document.removeEventListener("touchcancel", end);
    };
  }, [refreshing]);

  return <div
    className={`pull-refresh ${pull > 2 ? "is-visible" : ""} ${refreshing ? "is-refreshing" : ""}`}
    role="status"
    aria-label={t(refreshing ? "Refreshing" : "Pull to refresh")}
    style={{ "--pull": `${refreshing ? 44 : pull}px` } as React.CSSProperties}
  >
    {refreshing
      ? <LoaderCircle className="spin" />
      : <RefreshCw style={{ transform: `rotate(${Math.min(180, pull * 3)}deg)` }} />}
  </div>;
}
