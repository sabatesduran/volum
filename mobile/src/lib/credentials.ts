import { appDataDir } from "@tauri-apps/api/path";
import { invoke } from "@tauri-apps/api/core";
import { Stronghold } from "@tauri-apps/plugin-stronghold";
import type { Credential } from "../types";

const KEY = "paired-desktop";
let strongholdPromise: ReturnType<typeof openVault> | undefined;

async function openVault() {
  const directory = await appDataDir();
  const password = await invoke<string>("vault_password");
  const stronghold = await Stronghold.load(`${directory}/credentials.hold`, password);
  let client;
  try { client = await stronghold.loadClient("volum-companion"); }
  catch { client = await stronghold.createClient("volum-companion"); }
  return { stronghold, store: client.getStore() };
}

async function vault() {
  strongholdPromise ??= openVault();
  return strongholdPromise;
}

export async function loadCredential(): Promise<Credential | undefined> {
  const { store } = await vault();
  const value = await store.get(KEY);
  if (!value) return undefined;
  try { return JSON.parse(new TextDecoder().decode(new Uint8Array(value))) as Credential; }
  catch { return undefined; }
}

export async function saveCredential(credential: Credential): Promise<void> {
  const { stronghold, store } = await vault();
  await store.insert(KEY, Array.from(new TextEncoder().encode(JSON.stringify(credential))));
  await stronghold.save();
}

export async function clearCredential(): Promise<void> {
  const { stronghold, store } = await vault();
  await store.remove(KEY);
  await stronghold.save();
}
