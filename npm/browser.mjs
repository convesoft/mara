// @mara implements REQ-BROWSER-LOCAL-WORKSPACE
// @mara implements DES-JAVASCRIPT-CLIENT-ADAPTERS
import { createClient, clientError, MaraClientError } from './client.mjs';
import { snapshotJson } from './internal/json.mjs';
export { createClient, clientError, MaraClientError } from './client.mjs';

/** Owns one real bundled module worker; never retries or changes backend. */
export async function createWasmClient(options) {
  let snapshot;
  try {
    const input = snapshotJson(options);
    if (!input || Array.isArray(input) || Object.keys(input).length !== 1 ||
        !Object.hasOwn(input, 'snapshot') || !input.snapshot || Array.isArray(input.snapshot) ||
        typeof input.snapshot !== 'object') throw new Error('snapshot required');
    snapshot = input.snapshot;
  } catch {
    throw clientError('invalid_argument', 'invalid_argument', 'Supply { snapshot: <source snapshot v1> }; use an explicit empty snapshot for initialization');
  }
  let worker;
  try {
    worker = new Worker(new URL('./browser/worker.mjs', import.meta.url), { type: 'module' });
  } catch {
    throw clientError('unavailable', 'worker_unavailable', 'Unable to start the bundled Mara module worker');
  }
  const pending = new Map();
  let nextId = 0;
  let failed;
  let closed = false;
  const fail = () => {
    failed ??= clientError('unknown_outcome', 'worker_transport_failed',
      'Mara worker response was lost; dispatched work may have completed. Do not replay. Memory is unavailable; reload an explicitly selected snapshot in a new client.');
    for (const { reject } of pending.values()) reject(failed);
    pending.clear();
    worker.terminate();
  };
  worker.addEventListener('error', event => { event.preventDefault(); fail(); });
  worker.addEventListener('messageerror', fail);
  worker.addEventListener('message', ({ data }) => {
    if (!data || data.format_version !== 1 || !Number.isSafeInteger(data.id) ||
        !pending.has(data.id) || typeof data.ok !== 'boolean' ||
        (data.ok ? !Object.hasOwn(data, 'result') :
          data.error?.format_version !== 1 || typeof data.error?.error?.code !== 'string' ||
          typeof data.error?.error?.message !== 'string')) {
      fail();
      return;
    }
    const { resolve, reject } = pending.get(data.id);
    pending.delete(data.id);
    if (data.ok) resolve(data.result);
    else {
      const code = data.error.error.code;
      const kind = code === 'invalid_argument' ? 'invalid_argument'
        : code === 'unsupported_operation' || code === 'host_unavailable' || code === 'worker_startup_failed' ? 'unavailable'
        : code === 'worker_error' ? 'unknown_outcome' : 'operation';
      reject(new MaraClientError(kind, data.error));
    }
  });
  const adapter = {
    context: { backend: 'wasm', selection: {
      snapshot_format_version: snapshot.format_version, base_identity: snapshot.base_identity ?? null,
    } },
    execute(operation, params) {
      if (closed) return Promise.reject(clientError('closed', 'client_closed', 'Mara client is closed'));
      if (failed) return Promise.reject(failed);
      if (nextId === Number.MAX_SAFE_INTEGER) {
        return Promise.reject(clientError('unavailable', 'worker_ids_exhausted', 'Create a new client with an explicitly selected snapshot'));
      }
      return new Promise((resolve, reject) => {
        const id = ++nextId;
        pending.set(id, { resolve, reject });
        try { worker.postMessage({ format_version: 1, id, operation, params }); }
        catch {
          pending.delete(id);
          reject(clientError('transport', 'worker_dispatch_failed', 'Unable to dispatch input to the Mara worker; no replay was attempted'));
        }
      });
    },
    async close() {
      closed = true;
      worker.terminate();
      // Shared close drains prior calls. Setup failures also release worker ownership.
      for (const { reject } of pending.values()) reject(clientError('closed', 'client_closed', 'Mara client is closed'));
      pending.clear();
    },
  };
  const client = await createClient(adapter);
  try {
    await client.loadSnapshot(snapshot);
    return client;
  } catch (error) {
    try { await client.close(); } catch { /* retain original setup failure */ }
    throw error;
  }
}
