import type { CSSProperties } from "react";
import { t } from "../lib/i18n";

const silhouettes = ["lamp", "bracket", "planter", "comb", "iris", "tray", "mount", "index", "bird"];

export function ModelArt({ modelId, extension, missing = false }: { modelId: string; extension: string; missing?: boolean }) {
  const number = Number(modelId.match(/\d+/)?.[0] ?? 1);
  const silhouette = silhouettes[(number - 1) % silhouettes.length];
  const style = { "--art-hue": `${(number * 43 + 12) % 360}` } as CSSProperties;
  return (
    <div className={`model-art model-art--${silhouette} ${missing ? "is-missing" : ""}`} style={style}>
      <div className="model-art__shadow" />
      <div className="model-art__shape">
        <span /><span /><span /><span /><span />
      </div>
      {missing && <span className="model-art__offline">{t("Offline")}</span>}
      <span className="model-art__format">{extension}</span>
    </div>
  );
}
