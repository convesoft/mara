// Private addon adapter, not the downstream @convesoft/mara public facade.
import { createRequire } from 'node:module';
import path from 'node:path';

export class NativeClientError extends Error {
  constructor(failure) {
    super(failure.error.error.message);
    this.name = 'NativeClientError';
    this.kind = failure.kind;
    this.code = failure.error.error.code;
    this.envelope = failure.error;
  }
}

function unwrap(response) {
  if (!response.ok) throw new NativeClientError(response);
  return response.result;
}

/** Load one already-built addon by absolute path. No CLI or package discovery. */
export function loadAddon(addonPath) {
  if (Number(process.versions.node.split('.')[0]) < 24) throw new Error('Mara requires Node 24+');
  if (!path.isAbsolute(addonPath)) throw new Error('Supply an absolute addon path');
  const { NativeHandle } = createRequire(import.meta.url)(addonPath);
  return function createClient(options) {
    let handle;
    try {
      handle = new NativeHandle(options);
    } catch (error) {
      let failure;
      try { failure = JSON.parse(error.message); } catch { throw error; }
      throw new NativeClientError(failure);
    }
    let tail = Promise.resolve();
    let closing = false;
    const enqueue = work => {
      const result = tail.then(work);
      tail = result.catch(() => {});
      return result;
    };
    return {
      async execute(operation, params = {}) {
        if (closing) return Promise.reject(new NativeClientError({
          kind: 'closed', error: { format_version: 1,
            error: { code: 'client_closed', message: 'Native client is closed' } },
        }));
        // Snapshot at invocation, then preserve call order including close.
        const input = structuredClone(params);
        return enqueue(async () => unwrap(await handle.execute(operation, input)));
      },
      close() {
        if (closing) return tail;
        closing = true;
        return enqueue(async () => {
          unwrap(await handle.close());
          handle = null;
        });
      },
    };
  };
}
