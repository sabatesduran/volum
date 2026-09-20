import { useState } from "react";
import { ExternalLink, Link2, LoaderCircle } from "lucide-react";
import { useQueryClient } from "@tanstack/react-query";
import { api } from "../lib/tauri/api";
import { useAppStore } from "../app/store";
import type { WebSourcePreview } from "../types";
import { Dialog } from "./Dialog";

function providerName(provider: WebSourcePreview["provider"]) {
  return provider === "makerworld" ? "MakerWorld" : "Printables";
}

export function ImportFromWebDialog({ onClose }: { onClose: () => void }) {
  const queryClient = useQueryClient();
  const [url, setUrl] = useState("");
  const [preview, setPreview] = useState<WebSourcePreview>();
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  const [imageFailed, setImageFailed] = useState(false);
  const inspect = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!url.trim()) return;
    setLoading(true);
    setError("");
    setPreview(undefined);
    setImageFailed(false);
    try {
      setPreview(await api.previewWebSource(url.trim()));
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setLoading(false);
    }
  };
  const save = async (openPage: boolean) => {
    if (!preview) return;
    setSaving(true);
    setError("");
    try {
      const source = await api.saveWebSource(preview);
      await queryClient.invalidateQueries({ queryKey: ["web-sources"] });
      useAppStore.getState().selectView("web");
      if (openPage) await api.openExternal(source.canonicalUrl);
      onClose();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
      setSaving(false);
    }
  };
  return <Dialog title="Import from web" subtitle="Save a MakerWorld or Printables model, then attach its downloaded files." onClose={onClose} size="medium"><form className="web-import-form" onSubmit={inspect}><label className="field"><span>Model URL</span><div className="web-import-url"><Link2 size={17} /><input autoFocus type="url" required value={url} onChange={(event) => { setUrl(event.target.value); setPreview(undefined); setError(""); }} placeholder="https://www.printables.com/model/…" /><button className="button button--primary" disabled={loading}>{loading ? <LoaderCircle className="spin" size={15} /> : "Preview"}</button></div><small>Only public HTTPS model pages are requested. Volum never asks for your account credentials.</small></label>{error && <div className="form-error">{error}</div>}{preview && <article className="web-preview">{preview.imageUrl && !imageFailed ? <img src={preview.imageUrl} alt="" referrerPolicy="no-referrer" onError={() => setImageFailed(true)} /> : <span className={`web-preview__fallback is-${preview.provider}`}><Link2 size={24} /></span>}<div className="web-preview__copy"><span className={`provider-pill is-${preview.provider}`}>{providerName(preview.provider)}</span><h3>{preview.title}</h3>{preview.creator && <p>by {preview.creator}</p>}{preview.description && <small>{preview.description}</small>}<div className="web-preview__facts">{preview.license && <span>{preview.license}</span>}{preview.filamentGrams != null && <span>{preview.filamentGrams.toFixed(1)} g · default print profile</span>}{preview.remoteId && <span>Model {preview.remoteId}</span>}</div></div></article>}<div className="dialog__actions"><button type="button" className="button button--quiet" onClick={onClose}>Cancel</button>{preview && <><button type="button" className="button button--quiet" disabled={saving} onClick={() => void save(true)}><ExternalLink size={15} /> Save & open page</button><button type="button" className="button button--primary" disabled={saving} onClick={() => void save(false)}>{saving && <LoaderCircle className="spin" size={15} />} Save link</button></>}</div></form></Dialog>;
}
