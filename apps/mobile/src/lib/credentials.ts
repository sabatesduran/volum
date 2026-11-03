import { appDataDir } from "@tauri-apps/api/path";
import { invoke } from "@tauri-apps/api/core";
import { Stronghold } from "@tauri-apps/plugin-stronghold";
import type { Credential } from "../types";

const LEGACY_CLIENT = "volum-companion";
const LEGACY_KEY = "paired-desktop";

function parseCredential(value: string): Credential | undefined {
  try { return JSON.parse(value) as Credential; }
  catch { return undefined; }
}

async function storedCredential(): Promise<Credential | undefined> {
  const value = await invoke<string | null>("load_credential");
  return value ? parseCredential(value) : undefined;
}

async function migrateLegacyCredential(): Promise<Credential | undefined> {
  if (!await invoke<boolean>("legacy_credential_store_exists")) return undefined;

  const directory = (await appDataDir()).replace(/[\\/]$/, "");
  const password = await invoke<string>("vault_password");
  const stronghold = await Stronghold.load(`${directory}/credentials.hold`, password);
  try {
    const client = await stronghold.loadClient(LEGACY_CLIENT);
    const value = await client.getStore().get(LEGACY_KEY);
    if (!value) throw new Error("The legacy credential store does not contain a paired device.");
    const credential = parseCredential(new TextDecoder().decode(value));
    if (!credential) throw new Error("The legacy credential store is invalid.");

    await invoke("save_credential", { credential: JSON.stringify(credential) });
    const verified = await storedCredential();
    if (!verified || JSON.stringify(verified) !== JSON.stringify(credential)) {
      throw new Error("The migrated credential could not be verified.");
    }
    await stronghold.unload();
    await invoke("finalize_legacy_credential_migration");
    return credential;
  } catch (error) {
    await stronghold.unload().catch(() => undefined);
    throw error;
  }
}

export async function prepareCredentialStore(): Promise<void> {
  await invoke("prepare_credential_store");
}

export async function loadCredential(): Promise<Credential | undefined> {
  return await storedCredential() ?? await migrateLegacyCredential();
}

export async function saveCredential(credential: Credential): Promise<void> {
  await invoke("save_credential", { credential: JSON.stringify(credential) });
  if (await invoke<boolean>("legacy_credential_store_exists")) {
    await invoke("finalize_legacy_credential_migration");
  }
}

export async function clearCredential(): Promise<void> {
  await invoke("clear_credential");
  await invoke("clear_legacy_credential_store");
}
