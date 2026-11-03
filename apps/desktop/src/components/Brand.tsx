import volumIcon from "../assets/volum-icon-transparent.png";

export function Brand({ compact = false }: { compact?: boolean }) {
  return (
    <div className={`brand ${compact ? "brand--compact" : ""}`} aria-label="Volum">
      <img className="brand__mark" src={volumIcon} alt="" aria-hidden="true" />
      {!compact && <span className="brand__word">VOL<span>U</span>M</span>}
    </div>
  );
}
