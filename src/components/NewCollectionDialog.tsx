import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { Box, Gift, Hand, Sparkles, WandSparkles, Wrench } from "lucide-react";
import { useQuery } from "@tanstack/react-query";
import { api } from "../lib/tauri/api";
import { Dialog } from "./Dialog";

const symbols = [{ id: "box", Icon: Box }, { id: "sparkles", Icon: Sparkles }, { id: "gift", Icon: Gift }, { id: "wrench", Icon: Wrench }];
const colors = ["#ff5a36", "#2f8f64", "#8f6ac8", "#d59b2f", "#397aa8"];

export function NewCollectionDialog({ onClose }: { onClose: () => void }) {
  const queryClient = useQueryClient();
  const [name, setName] = useState("");
  const [symbol, setSymbol] = useState("box");
  const [color, setColor] = useState(colors[0]);
  const [saving, setSaving] = useState(false);
  const [smart, setSmart] = useState(false);
  const [tagId, setTagId] = useState("");
  const [format, setFormat] = useState("");
  const [availability, setAvailability] = useState<"" | "available" | "offline">("");
  const { data: tags = [] } = useQuery({ queryKey: ["tags"], queryFn: api.tags });
  const valid = Boolean(name.trim()) && (!smart || Boolean(tagId || format || availability));
  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!valid) return;
    setSaving(true);
    await api.createCollection({ name: name.trim(), symbol, color, smart, rule: smart ? { tagId: tagId || undefined, format: format || undefined, availability: availability || undefined } : undefined });
    await queryClient.invalidateQueries({ queryKey: ["collections"] });
    onClose();
  };
  return (
    <Dialog title="New collection" subtitle={smart ? "Smart collections update automatically as models change." : "Collections reference models without moving their files."} onClose={onClose}>
      <form className="dialog-form" onSubmit={submit}>
        <label className="field"><span>Name</span><input autoFocus value={name} onChange={(event) => setName(event.target.value)} placeholder="e.g. To print" /></label>
        <fieldset><legend>Collection type</legend><div className="collection-type-choice"><button type="button" className={!smart ? "is-active" : ""} onClick={() => setSmart(false)}><Hand size={16} /><span><strong>Manual</strong><small>Add models yourself</small></span></button><button type="button" className={smart ? "is-active" : ""} onClick={() => setSmart(true)}><WandSparkles size={16} /><span><strong>Smart</strong><small>Match rules automatically</small></span></button></div></fieldset>
        {smart && <fieldset className="smart-rules"><legend>Match all selected rules</legend><label className="field field--compact"><span>Tag</span><select value={tagId} onChange={(event) => setTagId(event.target.value)}><option value="">Any tag</option>{tags.map((tag) => <option key={tag.id} value={tag.id}>{tag.name}</option>)}</select></label><div className="calculator-grid"><label className="field field--compact"><span>Format</span><select value={format} onChange={(event) => setFormat(event.target.value)}><option value="">Any format</option>{["stl", "3mf", "obj", "step", "zip"].map((value) => <option key={value} value={value}>{value.toUpperCase()}</option>)}</select></label><label className="field field--compact"><span>Availability</span><select value={availability} onChange={(event) => setAvailability(event.target.value as typeof availability)}><option value="">Any</option><option value="available">Available</option><option value="offline">Offline</option></select></label></div>{!tagId && !format && !availability && <small className="form-hint form-hint--warning">Choose at least one rule.</small>}</fieldset>}
        <fieldset><legend>Symbol</legend><div className="choice-row">{symbols.map(({ id, Icon }) => <button type="button" key={id} className={`symbol-choice ${symbol === id ? "is-active" : ""}`} onClick={() => setSymbol(id)} aria-label={id}><Icon size={18} /></button>)}</div></fieldset>
        <fieldset><legend>Color</legend><div className="choice-row">{colors.map((value) => <button type="button" key={value} className={`color-choice ${color === value ? "is-active" : ""}`} style={{ background: value }} onClick={() => setColor(value)} aria-label={value} />)}</div></fieldset>
        <div className="dialog__actions"><button type="button" className="button button--quiet" onClick={onClose}>Cancel</button><button className="button button--primary" disabled={!valid || saving}>{saving ? "Creating…" : "Create collection"}</button></div>
      </form>
    </Dialog>
  );
}
