import { X } from "lucide-react";
import { useEffect } from "react";

export function Dialog({ title, subtitle, children, onClose, size = "small" }: { title: string; subtitle?: string; children: React.ReactNode; onClose: () => void; size?: "small" | "medium" }) {
  useEffect(() => {
    const close = (event: KeyboardEvent) => { if (event.key === "Escape") onClose(); };
    window.addEventListener("keydown", close);
    return () => window.removeEventListener("keydown", close);
  }, [onClose]);
  return (
    <div className="dialog-backdrop" role="presentation" onMouseDown={(event) => { if (event.currentTarget === event.target) onClose(); }}>
      <section className={`dialog dialog--${size}`} role="dialog" aria-modal="true" aria-labelledby="dialog-title">
        <button className="icon-button dialog__close" onClick={onClose} aria-label="Close"><X size={18} /></button>
        <header><h2 id="dialog-title">{title}</h2>{subtitle && <p>{subtitle}</p>}</header>
        {children}
      </section>
    </div>
  );
}
