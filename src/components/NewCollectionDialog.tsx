import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Box, Gift, Hand, Plus, Sparkles, Trash2, WandSparkles, Wrench } from "lucide-react";
import { api } from "../lib/tauri/api";
import { Dialog } from "./Dialog";
import { t } from "../lib/i18n";
import type { QueryRule, QueryRuleField } from "../types";

const symbols = [{ id: "box", Icon: Box }, { id: "sparkles", Icon: Sparkles }, { id: "gift", Icon: Gift }, { id: "wrench", Icon: Wrench }];
const colors = ["#ff5a36", "#2f8f64", "#8f6ac8", "#d59b2f", "#397aa8"];
const fields: Array<[QueryRuleField, string]> = [
  ["text", "Text"], ["format", "Format"], ["tag", "Tag"], ["folder", "Folder"], ["library", "Library"],
  ["availability", "Availability"], ["favorite", "Favorite"], ["webSource", "Web source"],
  ["parseStatus", "File status"], ["duplicate", "Duplicates"], ["added", "Date added"],
  ["modified", "Date modified"], ["opened", "Date opened"], ["fileCount", "File count"], ["width", "Width"],
  ["depth", "Depth"], ["height", "Height"]
];

function defaultRule(field: QueryRuleField = "format"): QueryRule {
  if (field === "format") return { field, operator: "is", value: "stl" };
  if (field === "availability") return { field, operator: "is", value: "available" };
  if (field === "favorite" || field === "webSource") return { field, operator: "is", value: true };
  if (field === "parseStatus") return { field, operator: "is", value: "ready" };
  if (field === "duplicate") return { field, operator: "is", value: "any" };
  if (["added", "modified", "opened"].includes(field)) return { field, operator: "after", value: new Date().toISOString().slice(0, 10) };
  if (["fileCount", "width", "depth", "height"].includes(field)) return { field, operator: "atLeast", value: 1 };
  return { field, operator: "is", value: "" };
}

function operators(field: QueryRuleField): Array<[QueryRule["operator"], string]> {
  if (["added", "modified", "opened"].includes(field)) return [["after", "After"], ["before", "Before"], ["on", "On"]];
  if (["fileCount", "width", "depth", "height"].includes(field)) return [["atLeast", "At least"], ["atMost", "At most"], ["is", "Exactly"]];
  if (["favorite", "webSource", "availability", "duplicate"].includes(field)) return [["is", "Is"]];
  return [["is", "Is"], ["isNot", "Is not"]];
}

export function NewCollectionDialog({ onClose }: { onClose: () => void }) {
  const queryClient = useQueryClient();
  const [name, setName] = useState("");
  const [symbol, setSymbol] = useState("box");
  const [color, setColor] = useState(colors[0]);
  const [saving, setSaving] = useState(false);
  const [smart, setSmart] = useState(false);
  const [matchMode, setMatchMode] = useState<"all" | "any">("all");
  const [rules, setRules] = useState<QueryRule[]>([defaultRule()]);
  const { data: tags = [] } = useQuery({ queryKey: ["tags"], queryFn: api.tags });
  const { data: folders = [] } = useQuery({ queryKey: ["folders"], queryFn: () => api.folders() });
  const { data: roots = [] } = useQuery({ queryKey: ["roots"], queryFn: api.roots });
  const valid = Boolean(name.trim()) && (!smart || (rules.length > 0 && rules.every((rule) => rule.value !== "")));
  const updateRule = (index: number, next: QueryRule) => setRules((current) => current.map((rule, item) => item === index ? next : rule));
  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!valid) return;
    setSaving(true);
    await api.createCollection({ name: name.trim(), symbol, color, smart, rule: smart ? { version: 1, matchMode, rules } : undefined });
    await queryClient.invalidateQueries({ queryKey: ["collections"] });
    onClose();
  };
  return (
    <Dialog title={t("New collection")} subtitle={t(smart ? "Smart collections update automatically as projects change." : "Collections reference projects without moving their files.")} onClose={onClose} size="medium">
      <form className="dialog-form" onSubmit={submit}>
        <label className="field"><span>{t("Name")}</span><input autoFocus value={name} onChange={(event) => setName(event.target.value)} placeholder={t("e.g. To print")} /></label>
        <fieldset><legend>{t("Collection type")}</legend><div className="collection-type-choice"><button type="button" className={!smart ? "is-active" : ""} onClick={() => setSmart(false)}><Hand size={16} /><span><strong>{t("Manual")}</strong><small>{t("Add projects yourself")}</small></span></button><button type="button" className={smart ? "is-active" : ""} onClick={() => setSmart(true)}><WandSparkles size={16} /><span><strong>{t("Smart")}</strong><small>{t("Match rules automatically")}</small></span></button></div></fieldset>
        {smart && <fieldset className="smart-rules"><legend>{t("Rules")}</legend><div className="segmented"><button type="button" className={matchMode === "all" ? "is-active" : ""} onClick={() => setMatchMode("all")}>{t("Match all")}</button><button type="button" className={matchMode === "any" ? "is-active" : ""} onClick={() => setMatchMode("any")}>{t("Match any")}</button></div>{rules.map((rule, index) => <div className="smart-rule-row" key={`${index}-${rule.field}`}><select aria-label={t("Field")} value={rule.field} onChange={(event) => updateRule(index, defaultRule(event.target.value as QueryRuleField))}>{fields.map(([value, label]) => <option value={value} key={value}>{t(label)}</option>)}</select><select aria-label={t("Operator")} value={rule.operator} onChange={(event) => updateRule(index, { ...rule, operator: event.target.value as QueryRule["operator"] })}>{operators(rule.field).map(([value, label]) => <option value={value} key={value}>{t(label)}</option>)}</select><RuleValue rule={rule} tags={tags} folders={folders} roots={roots} onChange={(value) => updateRule(index, { ...rule, value })} /><button type="button" className="icon-button icon-button--tiny" aria-label={t("Remove rule")} onClick={() => setRules((current) => current.filter((_, item) => item !== index))}><Trash2 size={14} /></button></div>)}<button type="button" className="button button--quiet" onClick={() => setRules((current) => [...current, defaultRule()])}><Plus size={14} /> {t("Add rule")}</button>{rules.length === 0 && <small className="form-hint form-hint--warning">{t("Choose at least one rule.")}</small>}</fieldset>}
        <fieldset><legend>{t("Symbol")}</legend><div className="choice-row">{symbols.map(({ id, Icon }) => <button type="button" key={id} className={`symbol-choice ${symbol === id ? "is-active" : ""}`} onClick={() => setSymbol(id)} aria-label={id}><Icon size={18} /></button>)}</div></fieldset>
        <fieldset><legend>{t("Color")}</legend><div className="choice-row">{colors.map((value) => <button type="button" key={value} className={`color-choice ${color === value ? "is-active" : ""}`} style={{ background: value }} onClick={() => setColor(value)} aria-label={value} />)}</div></fieldset>
        <div className="dialog__actions"><button type="button" className="button button--quiet" onClick={onClose}>{t("Cancel")}</button><button className="button button--primary" disabled={!valid || saving}>{t(saving ? "Creating…" : "Create collection")}</button></div>
      </form>
    </Dialog>
  );
}

function RuleValue({ rule, tags, folders, roots, onChange }: { rule: QueryRule; tags: Array<{ id: string; name: string }>; folders: Array<{ id: string; relativePath: string }>; roots: Array<{ id: string; displayName: string }>; onChange: (value: string | number | boolean) => void }) {
  if (rule.field === "format") return <select aria-label={t("Value")} value={String(rule.value)} onChange={(event) => onChange(event.target.value)}>{["stl", "3mf", "obj", "step"].map((value) => <option value={value} key={value}>{value.toUpperCase()}</option>)}</select>;
  if (rule.field === "tag") return <select aria-label={t("Value")} value={String(rule.value)} onChange={(event) => onChange(event.target.value)}><option value="">{t("Choose tag")}</option>{tags.map((tag) => <option value={tag.id} key={tag.id}>{tag.name}</option>)}</select>;
  if (rule.field === "folder") return <select aria-label={t("Value")} value={String(rule.value)} onChange={(event) => onChange(event.target.value)}><option value="">{t("Choose folder")}</option>{folders.map((folder) => <option value={folder.id} key={folder.id}>{folder.relativePath}</option>)}</select>;
  if (rule.field === "library") return <select aria-label={t("Value")} value={String(rule.value)} onChange={(event) => onChange(event.target.value)}><option value="">{t("Choose library")}</option>{roots.map((root) => <option value={root.id} key={root.id}>{root.displayName}</option>)}</select>;
  if (rule.field === "availability") return <select aria-label={t("Value")} value={String(rule.value)} onChange={(event) => onChange(event.target.value)}><option value="available">{t("Available")}</option><option value="offline">{t("Offline")}</option></select>;
  if (rule.field === "favorite" || rule.field === "webSource") return <select aria-label={t("Value")} value={String(rule.value)} onChange={(event) => onChange(event.target.value === "true")}><option value="true">{t("Yes")}</option><option value="false">{t("No")}</option></select>;
  if (rule.field === "parseStatus") return <select aria-label={t("Value")} value={String(rule.value)} onChange={(event) => onChange(event.target.value)}><option value="ready">{t("Ready")}</option><option value="warning">{t("Warning")}</option></select>;
  if (rule.field === "duplicate") return <select aria-label={t("Value")} value={String(rule.value)} onChange={(event) => onChange(event.target.value)}><option value="any">{t("Any duplicate")}</option><option value="exact">{t("Exact file")}</option><option value="geometry">{t("Same geometry")}</option><option value="none">{t("No duplicate")}</option></select>;
  if (["added", "modified", "opened"].includes(rule.field)) return <input aria-label={t("Value")} type="date" value={String(rule.value)} onChange={(event) => onChange(event.target.value)} />;
  if (["fileCount", "width", "depth", "height"].includes(rule.field)) return <input aria-label={t("Value")} type="number" min="0" step={rule.field === "fileCount" ? 1 : .1} value={Number(rule.value)} onChange={(event) => onChange(Number(event.target.value))} />;
  return <input aria-label={t("Value")} value={String(rule.value)} onChange={(event) => onChange(event.target.value)} />;
}
