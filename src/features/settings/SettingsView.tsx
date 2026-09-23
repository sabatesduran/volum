import { useEffect, useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Check, Download, FolderOpen, FolderPlus, HardDrive, Moon, MoreHorizontal, Package, Plus, RefreshCw, Star, Sun, Trash2 } from "lucide-react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { useAppStore } from "../../app/store";
import { api, isTauri } from "../../lib/tauri/api";
import { formatDate } from "../../lib/format";
import { emptySlicerConfig, parseSlicerConfig, stringifySlicerConfig } from "../../lib/slicers";
import { SlicerIcon } from "../../components/SlicerIcon";
import { AboutSettings } from "../../components/AboutSettings";
import { Dialog } from "../../components/Dialog";
import type { LanguagePreference, SlicerConfig, Theme } from "../../types";
import { intlLocale, plural, t } from "../../lib/i18n";
import { UpdateSettings } from "../updates/UpdateSettings";
import { BackupSettings } from "../backups/BackupSettings";

function hexToRgb(hex: string): [number, number, number] {
  const value = Number.parseInt(hex.replace("#", ""), 16);
  return [(value >> 16) & 255, (value >> 8) & 255, value & 255];
}

function ColorPicker({ value, onChange }: { value: string; onChange: (value: string) => void }) {
  return (
    <div className="material-color-picker">
      <label className="color-well" title={t("Choose color")}><input type="color" value={value} onChange={(event) => onChange(event.target.value)} aria-label={t("Choose material color")} /><span style={{ background: value }} /></label>
    </div>
  );
}

export function SettingsView() {
  const queryClient = useQueryClient();
  const { theme, setTheme, language, setLanguage } = useAppStore();
  const { data: roots = [] } = useQuery({ queryKey: ["roots"], queryFn: api.roots });
  const { data: materials = [] } = useQuery({ queryKey: ["materials"], queryFn: api.materials });
  const { data: preferences = {} } = useQuery({ queryKey: ["preferences"], queryFn: api.preferences });
  const storedSlicerConfig = useMemo(() => parseSlicerConfig(preferences), [preferences]);
  const [slicerConfig, setSlicerConfig] = useState<SlicerConfig>(emptySlicerConfig);
  useEffect(() => setSlicerConfig(storedSlicerConfig), [storedSlicerConfig]);
  const customSlicerKey = slicerConfig.customApps.map((app) => `${app.id}:${app.path}`).join("|");
  const { data: slicers = [] } = useQuery({ queryKey: ["slicers", customSlicerKey], queryFn: () => api.slicers(slicerConfig.customApps) });
  const enabledSlicerApplications = useMemo(
    () => slicers.filter((slicer) => slicerConfig.enabledIds.includes(slicer.id)),
    [slicers, slicerConfig.enabledIds]
  );
  const [materialName, setMaterialName] = useState("");
  const [price, setPrice] = useState("15.99");
  const [materialColor, setMaterialColor] = useState("#e9e2d3");
  const [activeSection, setActiveSection] = useState("libraries");
  const [slicerPickerOpen, setSlicerPickerOpen] = useState(false);
  const [webImportFolderError, setWebImportFolderError] = useState("");
  const addRoot = async () => {
    const path = isTauri() ? await open({ directory: true, multiple: false, title: t("Choose a 3D model folder") }) : "/Users/you/New Models";
    if (!path || Array.isArray(path)) return;
    const root = await api.addRoot(path);
    await queryClient.invalidateQueries({ queryKey: ["roots"] });
    await api.startScan(root.id);
  };
  const reconnectRoot = async (rootId: string) => {
    const path = isTauri() ? await open({ directory: true, multiple: false, title: t("Reconnect model library") }) : "/Users/you/3D Models";
    if (!path || Array.isArray(path)) return;
    await api.reconnectRoot(rootId, path);
    await queryClient.invalidateQueries({ queryKey: ["roots"] });
    await api.startScan(rootId);
  };
  const chooseWebImportFolder = async () => {
    if (!isTauri()) return;
    const path = await open({ directory: true, multiple: false, title: t("Choose the Web imports folder") });
    if (!path || Array.isArray(path)) return;
    setWebImportFolderError("");
    try {
      await api.savePreference("web_import_folder", path);
      await queryClient.invalidateQueries({ queryKey: ["preferences"] });
    } catch (reason) {
      setWebImportFolderError(reason instanceof Error ? reason.message : String(reason));
    }
  };
  const saveMaterial = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!materialName.trim()) return;
    await api.saveMaterial({ name: materialName, materialType: "PLA", colorHex: materialColor, spoolPriceMinor: Math.round(Number(price) * 100), currency: "EUR", spoolWeightG: 1000 });
    setMaterialName("");
    queryClient.invalidateQueries({ queryKey: ["materials"] });
  };
  const updateSlicerConfig = async (next: SlicerConfig) => {
    setSlicerConfig(next);
    await api.savePreference("slicer_config", stringifySlicerConfig(next));
    await queryClient.invalidateQueries({ queryKey: ["preferences"] });
  };
  const toggleSlicer = async (id: string) => {
    const enabled = slicerConfig.enabledIds.includes(id);
    const enabledIds = enabled ? slicerConfig.enabledIds.filter((item) => item !== id) : [...slicerConfig.enabledIds, id];
    const defaultId = enabled && slicerConfig.defaultId === id ? enabledIds[0] : slicerConfig.defaultId ?? id;
    await updateSlicerConfig({ ...slicerConfig, enabledIds, defaultId });
  };
  const enableSlicer = async (id: string) => {
    if (slicerConfig.enabledIds.includes(id)) return;
    const enabledIds = [...slicerConfig.enabledIds, id];
    await updateSlicerConfig({ ...slicerConfig, enabledIds, defaultId: slicerConfig.defaultId ?? id });
    setSlicerPickerOpen(false);
  };
  const chooseSlicer = async () => {
    if (!isTauri()) return;
    const path = await open({ directory: false, multiple: false, title: t("Choose your slicer application") });
    if (!path || Array.isArray(path)) return;
    const name = path.split(/[\\/]/).pop()?.replace(/\.(app|exe)$/i, "") ?? "Slicer";
    const existing = slicerConfig.customApps.find((app) => app.path === path);
    const application = existing ?? { id: `custom-${crypto.randomUUID()}`, name, path };
    const customApps = existing ? slicerConfig.customApps : [...slicerConfig.customApps, application];
    const enabledIds = [...new Set([...slicerConfig.enabledIds, application.id])];
    await updateSlicerConfig({ ...slicerConfig, customApps, enabledIds, defaultId: slicerConfig.defaultId ?? application.id });
    setSlicerPickerOpen(false);
  };
  const removeCustomSlicer = async (id: string) => {
    const enabledIds = slicerConfig.enabledIds.filter((item) => item !== id);
    await updateSlicerConfig({
      customApps: slicerConfig.customApps.filter((app) => app.id !== id),
      enabledIds,
      defaultId: slicerConfig.defaultId === id ? enabledIds[0] : slicerConfig.defaultId
    });
  };
  const exportMetadata = async () => {
    if (!isTauri()) return;
    const path = await save({ title: t("Export Volum metadata"), defaultPath: "volum-metadata.json", filters: [{ name: "JSON", extensions: ["json"] }] });
    if (path) await api.exportMetadata(path);
  };
  const exportDiagnostics = async () => {
    if (!isTauri()) return;
    const path = await save({ title: t("Export redacted diagnostics"), defaultPath: "volum-diagnostics.json", filters: [{ name: "JSON", extensions: ["json"] }] });
    if (path) await api.exportDiagnostics(path, true);
  };
  return (
    <section className="content-view settings-view">
      <header className="view-header"><div><div className="eyebrow">{t("Volum preferences")}</div><h1>{t("Settings")}</h1><p>{t("Libraries, web imports, backups, materials, appearance, indexing, and about.")}</p></div></header>
      <div className="settings-layout">
        <nav className="settings-nav">{[["libraries", "Libraries"], ["web-imports", "Web imports"], ["appearance", "Appearance"], ["materials", "Materials"], ["applications", "Applications"], ["privacy", "Privacy & data"], ["backups", "Backups"], ["updates", "Updates"], ["about", "About"]].map(([id, label]) => <a key={id} href={`#${id}`} className={activeSection === id ? "is-active" : ""} onClick={() => setActiveSection(id)}>{t(label)}</a>)}</nav>
        <div className="settings-content">
          <section className="settings-section" id="libraries"><div className="settings-section__header"><div><h2>{t("Libraries")}</h2><p>{t("Volum watches these folders. Your files stay exactly where they are.")}</p></div><button className="button button--primary" onClick={addRoot}><FolderPlus size={16} /> {t("Add folder")}</button></div><div className="root-list">{roots.map((root) => <div className="root-row" key={root.id}><span className={`root-icon root-icon--${root.status}`}><HardDrive size={20} /></span><div><strong>{root.displayName}</strong><small>{root.path}</small><small>{plural(root.modelCount, "{count} model", "{count} models")} · {t("Scanned {date}", { date: formatDate(root.lastScanAt) })}</small></div><span className={`status-pill status-pill--${root.status}`}>{t(root.status === "online" ? "Online" : root.status === "offline" ? "Offline" : root.status === "scanning" ? "Scanning" : root.status === "paused" ? "Paused" : "Error")}</span>{root.status === "offline" ? <button className="button button--quiet" onClick={() => void reconnectRoot(root.id)}><FolderOpen size={14} /> {t("Reconnect")}</button> : <button className="icon-button" onClick={() => api.startScan(root.id)} aria-label={t("Rescan")}><RefreshCw size={16} /></button>}<button className="icon-button" onClick={async () => { await api.removeRoot(root.id); queryClient.invalidateQueries({ queryKey: ["roots"] }); }} aria-label={t("Remove library")}><Trash2 size={16} /></button></div>)}</div></section>
          <section className="settings-section" id="web-imports"><div className="settings-section__header"><div><h2>{t("Web imports")}</h2><p>{t("Downloaded files are copied here before Volum indexes them.")}</p></div><button className="button button--quiet" onClick={chooseWebImportFolder}><FolderOpen size={15} /> {t("Choose folder")}</button></div><div className="web-import-folder-setting"><span><Download size={19} /></span><div><strong>{t(preferences.web_import_folder ? "Custom import folder" : "Default import folder")}</strong><small>{preferences.web_import_folder || (roots[0] ? `${roots[0].path}/Web Imports` : t("Add a library to configure imports"))}</small></div>{preferences.web_import_folder && <button className="button button--quiet" onClick={async () => { await api.savePreference("web_import_folder", ""); await queryClient.invalidateQueries({ queryKey: ["preferences"] }); }}>{t("Use default")}</button>}</div>{webImportFolderError && <div className="form-error">{webImportFolderError}</div>}<p className="settings-footnote">{t("The selected folder must be inside an indexed library. Each import is organized by provider and model.")}</p></section>
          <section className="settings-section" id="appearance"><div className="settings-section__header"><div><h2>{t("Appearance")}</h2><p>{t("Use your system setting or choose a theme.")}</p></div></div><div className="theme-choices">{([["system", MoreHorizontal], ["light", Sun], ["dark", Moon]] as Array<[Theme, typeof Sun]>).map(([value, Icon]) => <button key={value} className={theme === value ? "is-active" : ""} onClick={() => setTheme(value)}><span><Icon size={21} /></span><strong>{t(value === "system" ? "System" : value === "light" ? "Light" : "Dark")}</strong></button>)}</div><div className="language-setting"><div><h3>{t("Language")}</h3><p>{t("Follow the operating system language, or override it for Volum.")}</p></div><label className="field field--compact"><span>{t("App language")}</span><select value={language} onChange={(event) => setLanguage(event.target.value as LanguagePreference)}><option value="system">{t("System")}</option><option value="en">English</option><option value="ca">Català</option><option value="es">Español (España)</option></select></label></div></section>
          <section className="settings-section" id="materials"><div className="settings-section__header"><div><h2>{t("Materials")}</h2><p>{t("Price presets for estimates—not spool inventory.")}</p></div></div><div className="material-list">{materials.map((material) => <div className="material-row" key={material.id}><span className="material-swatch" style={{ background: material.colorHex ?? "#e9e2d3" }} /><div><strong>{material.name}</strong><small>{material.materialType} · {material.colorHex ? `RGB ${hexToRgb(material.colorHex).join(", ")} · ` : ""}{material.spoolWeightG.toLocaleString(intlLocale)} g</small></div><strong>{new Intl.NumberFormat(intlLocale, { style: "currency", currency: material.currency }).format(material.spoolPriceMinor / 100)}</strong></div>)}</div><form className="material-form" onSubmit={saveMaterial}><div className="material-form__main"><ColorPicker value={materialColor} onChange={setMaterialColor} /><input value={materialName} onChange={(event) => setMaterialName(event.target.value)} placeholder={t("Material name")} aria-label={t("Material name")} /><div className="input-unit input-unit--prefix"><span>€</span><input type="number" value={price} onChange={(event) => setPrice(event.target.value)} step=".01" aria-label={t("Spool price")} /></div><button className="button button--quiet">{t("Add preset")}</button></div></form></section>
          <section className="settings-section" id="applications"><div className="settings-section__header"><div><h2>{t("Slicer applications")}</h2><p>{t("Enabled slicers available from model actions.")}</p></div><button className="button button--quiet" onClick={() => setSlicerPickerOpen(true)}><Plus size={15} /> {t("Add")}</button></div><div className="slicer-settings-list">{enabledSlicerApplications.map((slicer) => { const isDefault = slicerConfig.defaultId === slicer.id; return <div className={`slicer-settings-row ${!slicer.installed ? "is-unavailable" : ""}`} key={slicer.id}><SlicerIcon application={slicer} size={34} /><div className="slicer-settings-row__copy"><strong>{slicer.name}</strong><small>{slicer.installed ? slicer.path : t("Not installed")}</small></div><button className="slicer-enable is-enabled" onClick={() => toggleSlicer(slicer.id)} aria-label={t("Disable {name}", { name: slicer.name })}><span><Check size={12} /></span>{t("Enabled")}</button><button className={`slicer-default ${isDefault ? "is-default" : ""}`} disabled={!slicer.installed} onClick={() => updateSlicerConfig({ ...slicerConfig, defaultId: slicer.id })}><Star size={13} fill={isDefault ? "currentColor" : "none"} />{t(isDefault ? "Default" : "Make default")}</button>{slicer.custom && <button className="icon-button icon-button--tiny" onClick={() => removeCustomSlicer(slicer.id)} aria-label={t("Remove {name}", { name: slicer.name })}><Trash2 size={14} /></button>}</div>; })}{enabledSlicerApplications.length === 0 && <div className="slicer-settings-empty">{t("No slicers are enabled. Add a slicer to use it from model actions.")}</div>}</div><p className="settings-footnote">{t("Only enabled slicers are shown here and in the Open menu.")}</p></section>
          <section className="settings-section" id="privacy"><div className="settings-section__header"><div><h2>{t("Privacy & data")}</h2><p>{t("Volum has no account, analytics, or model uploads. Its index and previews remain on this computer.")}</p></div></div><div className="privacy-note"><Package size={20} /><span><strong>{t("Local by default")}</strong><small>{t("Network access is used for update checks, About projects, and public MakerWorld or Printables metadata only when you preview a link. No library information is sent.")}</small></span></div><div className="settings-actions"><button className="button button--quiet" onClick={exportMetadata}>{t("Export metadata")}</button><button className="button button--quiet" onClick={exportDiagnostics}>{t("Export diagnostics")}</button><button className="button button--quiet" onClick={() => roots.forEach((root) => api.startScan(root.id))}><RefreshCw size={15} /> {t("Rebuild index")}</button></div></section>
          <BackupSettings />
          <UpdateSettings />
          <AboutSettings />
        </div>
      </div>
      {slicerPickerOpen && <Dialog title={t("Add slicer")} subtitle={t("Choose a supported slicer or select a custom application.")} onClose={() => setSlicerPickerOpen(false)}><div className="slicer-picker-list">{slicers.filter((slicer) => !slicerConfig.enabledIds.includes(slicer.id)).map((slicer) => <button key={slicer.id} className="slicer-picker-row" disabled={!slicer.installed} onClick={() => enableSlicer(slicer.id)}><SlicerIcon application={slicer} size={36} /><span><strong>{slicer.name}</strong><small>{slicer.installed ? slicer.path : t("Not installed")}</small></span><span className="slicer-picker-row__action">{t("Add")}</span></button>)}<button className="slicer-picker-row slicer-picker-row--custom" onClick={chooseSlicer}><span className="slicer-picker-row__custom-icon"><Plus size={18} /></span><span><strong>{t("Custom application…")}</strong><small>{t("Choose another slicer from this computer")}</small></span><span className="slicer-picker-row__action">{t("Choose")}</span></button></div></Dialog>}
    </section>
  );
}
