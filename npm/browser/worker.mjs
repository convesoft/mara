// @mara implements REQ-BROWSER-LOCAL-WORKSPACE
// Dedicated module worker. Only trusted bundled assets are fetched; source inputs are messages.
const ready = import('./mara_wasm.js').then(async ({ default: init, Workspace }) => {
  await init();
  return new Workspace();
});
// Report startup failure through correlated responses, even if loading fails before a message.
ready.catch(() => {});
// Serializes even requests received while WASM is still loading.
let queue = Promise.resolve();
self.onmessage = ({ data }) => {
  queue = queue.then(async () => {
    const id = data?.id ?? null;
    try {
      if (!data || data.format_version !== 1 || !Number.isSafeInteger(data.id) ||
          typeof data.operation !== 'string') {
        throw new Error('expected worker request v1 with integer id and operation');
      }
      let workspace;
      try { workspace = await ready; }
      catch {
        self.postMessage({ format_version: 1, id, ok: false, error: {
          format_version: 1, error: { code: 'worker_startup_failed', message: 'Unable to initialize the bundled Mara WASM engine' },
        } });
        return;
      }
      const response = JSON.parse(workspace.request(JSON.stringify({
        operation: data.operation, params: data.params ?? {},
      })));
      self.postMessage({ format_version: 1, id, ...response });
    } catch (error) {
      self.postMessage({ format_version: 1, id, ok: false, error: {
        format_version: 1, error: { code: 'worker_error', message: String(error.message ?? error) },
      } });
    }
  });
};
