import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { Download, ExternalLink, FilePlus2, Globe2, Link2, LoaderCircle, Plus, Trash2 } from "lucide-react";
import { useAppStore } from "../../app/store";
import { Dialog } from "../../components/Dialog";
import { api, isTauri } from "../../lib/tauri/api";
import type { WebSource } from "../../types";
import { t } from "../../lib/i18n";
import { formatNumber } from "../../lib/format";

function providerName(provider: WebSource["provider"]) {
  return provider === "makerworld" ? "MakerWorld" : "Printables";
}

export function WebImportsView({ onImport }: { onImport: () => void }) {
  const queryClient = useQueryClient();
  const selectModel = useAppStore((state) => state.selectModel);
  const { data: sources = [], isLoading } = useQuery({ queryKey: ["web-sources"], queryFn: api.webSources });
  const { data: roots = [] } = useQuery({ queryKey: ["roots"], queryFn: api.roots });
  const { data: preferences = {} } = useQuery({ queryKey: ["preferences"], queryFn: api.preferences });
  const [attaching, setAttaching] = useState<WebSource>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const beginAttach = (source: WebSource) => {
    setAttaching(source);
    setError("");
  };
  const chooseAndAttach = async () => {
    if (!attaching) return;
    const path = isTauri() ? await open({ multiple: false, directory: false, title: t("Attach a downloaded file to {title}", { title: attaching.title }), filters: [{ name: t("3D model files"), extensions: ["3mf", "stl", "obj", "step", "stp"] }] }) : "/Users/you/Downloads/model.3mf";
    if (!path || Array.isArray(path)) return;
    setBusy(true);
    setError("");
    try {
      await api.attachWebSource(attaching.id, path);
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["web-sources"] }),
        queryClient.invalidateQueries({ queryKey: ["models"] })
      ]);
      setAttaching(undefined);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setBusy(false);
    }
  };
  const remove = async (source: WebSource) => {
    if (!window.confirm(t("Remove “{title}” from Web imports? Local model files will not be deleted.", { title: source.title }))) return;
    await api.deleteWebSource(source.id);
    await queryClient.invalidateQueries({ queryKey: ["web-sources"] });
  };
  const importFolder = preferences.web_import_folder || (roots[0] ? `${roots[0].path}/Web Imports` : t("Configure an import folder in Settings"));
  return <section className="content-view web-imports-view"><header className="view-header"><div><div className="eyebrow">MakerWorld & Printables</div><h1>{t("Web imports")}</h1><p>{t("Keep attribution and downloaded models together.")}</p></div><button className="button button--primary" onClick={onImport}><Plus size={16} /> {t("Import link")}</button></header>{isLoading ? <div className="web-source-grid">{Array.from({ length: 4 }, (_, index) => <div className="web-source-card skeleton-card" key={index}><div className="web-source-card__image skeleton" /></div>)}</div> : sources.length === 0 ? <div className="empty-state"><span className="empty-state__icon"><Globe2 size={24} /></span><h2>{t("No web models saved")}</h2><p>{t("Paste a MakerWorld or Printables link to start an import.")}</p><button className="button button--primary" onClick={onImport}><Link2 size={15} /> {t("Import your first link")}</button></div> : <div className="web-source-grid">{sources.map((source) => <article className="web-source-card" key={source.id}><div className="web-source-card__image">{source.imageUrl ? <img src={source.imageUrl} alt="" loading="lazy" referrerPolicy="no-referrer" /> : <span className={`web-source-card__fallback is-${source.provider}`}><Globe2 size={26} /></span>}<span className={`provider-pill is-${source.provider}`}>{providerName(source.provider)}</span><span className={`web-source-status is-${source.status}`}>{t(source.status === "imported" ? "In library" : source.status === "importing" ? "Indexing" : "Saved link")}</span></div><div className="web-source-card__body"><h2>{source.title}</h2>{source.creator && <p>{t("by {name}", { name: source.creator })}</p>}{source.description && <small>{source.description}</small>}<div className="web-source-card__facts">{source.license && <span>{source.license}</span>}{source.filamentGrams != null && <span>{formatNumber(source.filamentGrams, { minimumFractionDigits: 1, maximumFractionDigits: 1 })} g · {t("default print profile")}</span>}{source.relativePath && <span>{source.relativePath}</span>}</div></div><div className="web-source-card__actions"><button className="button button--quiet" onClick={() => api.openExternal(source.canonicalUrl)}><ExternalLink size={14} /> {t("Source")}</button>{source.modelId ? <button className="button button--primary" onClick={() => selectModel(source.modelId)}><Download size={14} /> {t("View model")}</button> : source.status === "importing" ? <button className="button button--quiet" disabled><LoaderCircle className="spin" size={14} /> {t("Indexing")}</button> : <button className="button button--primary" onClick={() => beginAttach(source)}><FilePlus2 size={14} /> {t("Attach file")}</button>}<button className="icon-button" aria-label={t("Remove {name}", { name: source.title })} onClick={() => void remove(source)}><Trash2 size={15} /></button></div></article>)}</div>}{attaching && <Dialog title={t("Attach downloaded file")} subtitle={t("Volum will copy the file into your configured import folder and index it normally.")} onClose={() => !busy && setAttaching(undefined)}><div className="attach-web-source"><div className="attach-web-source__model"><span className={`provider-pill is-${attaching.provider}`}>{providerName(attaching.provider)}</span><strong>{attaching.title}</strong></div><div className="web-import-destination"><span>{t("Destination")}</span><strong>{importFolder}</strong><small>{t("The original download stays where it is. Change this folder in Settings.")}</small></div>{error && <div className="form-error">{error}</div>}<div className="dialog__actions"><button className="button button--quiet" disabled={busy} onClick={() => setAttaching(undefined)}>{t("Cancel")}</button><button className="button button--primary" disabled={busy || !roots.length} onClick={() => void chooseAndAttach()}>{busy ? <LoaderCircle className="spin" size={15} /> : <FilePlus2 size={15} />} {t("Choose file")}</button></div></div></Dialog>}</section>;
}
