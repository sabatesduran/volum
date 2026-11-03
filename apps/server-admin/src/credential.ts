export interface WebCredential {
  serverId: string;
  serverName: string;
  deviceId: string;
  token: string;
}

const DATABASE = "volum-web";
const STORE = "credentials";
const KEY = "administrator";

function database(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DATABASE, 1);
    request.onupgradeneeded = () => {
      if (!request.result.objectStoreNames.contains(STORE)) {
        request.result.createObjectStore(STORE);
      }
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("Browser storage is unavailable."));
  });
}

function operation<T>(mode: IDBTransactionMode, run: (store: IDBObjectStore) => IDBRequest<T>) {
  return database().then((db) => new Promise<T>((resolve, reject) => {
    const transaction = db.transaction(STORE, mode);
    const request = run(transaction.objectStore(STORE));
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("Browser storage could not be updated."));
    transaction.oncomplete = () => db.close();
    transaction.onerror = () => reject(transaction.error ?? new Error("Browser storage could not be updated."));
  }));
}

export function loadCredential() {
  return operation<WebCredential | undefined>("readonly", (store) => store.get(KEY));
}

export function saveCredential(credential: WebCredential) {
  return operation("readwrite", (store) => store.put(credential, KEY));
}

export function clearCredential() {
  return operation("readwrite", (store) => store.delete(KEY));
}
