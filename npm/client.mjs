// @mara implements REQ-SHARED-TYPESCRIPT-CLIENT
// @mara implements DES-JAVASCRIPT-CLIENT-ADAPTERS
// Browser-safe facade: adapters own setup, transport and domain semantics.
export class MaraClientError extends Error {
  constructor(kind, envelope) {
    super(envelope.error.message);
    this.name = 'MaraClientError';
    this.kind = kind;
    this.code = envelope.error.code;
    this.envelope = envelope;
  }
}

export function clientError(kind, code, message) {
  return new MaraClientError(kind, { format_version: 1, error: { code, message } });
}

const methods = {
  projectInit: 'project_init', projectMidBackfill: 'project_mid_backfill',
  projectTransactionRollback: 'project_transaction_rollback', projectRefresh: 'project_refresh',
  schemaGet: 'schema_get', schemaList: 'schema_list', schemaValidate: 'schema_validate',
  projectValidate: 'project_validate', itemValidate: 'item_validate', itemList: 'item_list',
  search: 'search', get: 'get', related: 'related', itemCreate: 'item_create',
  itemUpdate: 'item_update', itemMove: 'item_move', itemDelete: 'item_delete', itemRename: 'item_rename',
  relationGet: 'relation_get', relationAdd: 'relation_add', relationRemove: 'relation_remove',
  traceMatrix: 'trace_matrix',
};

function snapshot(value) {
  // Copy JSON data without letting stringify/toJSON turn invalid inputs into valid mutations.
  // Undefined optional object properties mean omission; array entries must be JSON values.
  const active = new Set();
  const copy = entry => {
    if (entry === null || typeof entry === 'string' || typeof entry === 'boolean') return entry;
    if (typeof entry === 'number' && Number.isFinite(entry)) return entry;
    if (typeof entry !== 'object' || active.has(entry)) throw new Error('Not JSON data');
    const prototype = Object.getPrototypeOf(entry);
    if (!Array.isArray(entry) && prototype !== null && Object.getPrototypeOf(prototype) !== null) {
      throw new Error('Not a JSON object');
    }
    if (Object.getOwnPropertySymbols(entry).some(key => Object.prototype.propertyIsEnumerable.call(entry, key))) {
      throw new Error('Not a JSON key');
    }
    active.add(entry);
    const result = Array.isArray(entry) ? [] : Object.fromEntries(
      Object.entries(entry).filter(([, child]) => child !== undefined).map(([key, child]) => [key, copy(child)]));
    if (Array.isArray(entry)) {
      const length = entry.length;
      for (let index = 0; index < length; index++) result.push(copy(entry[index]));
    }
    active.delete(entry);
    return result;
  };
  try { return copy(value); }
  catch { throw clientError('invalid_argument', 'invalid_argument', 'Supply JSON-compatible client inputs'); }
}

function freeze(value) {
  if (value && typeof value === 'object') {
    for (const child of Object.values(value)) freeze(child);
    Object.freeze(value);
  }
  return value;
}

/** Takes ownership of the supplied adapter; never retries or selects another backend. */
export async function createClient(adapter) {
  let capabilities;
  let context;
  try {
    context = freeze(snapshot(adapter.context));
    capabilities = freeze(snapshot(await adapter.execute('capabilities', {})));
  } catch (error) {
    // A failed setup must release its adapter, but not obscure the setup failure.
    try { await adapter.close(); } catch { /* original failure wins */ }
    throw error;
  }
  let tail = Promise.resolve();
  let closing;
  const enqueue = work => {
    const result = tail.then(work);
    tail = result.catch(() => {});
    return result;
  };
  const client = { context, capabilities,
    close() {
      if (!closing) closing = enqueue(() => adapter.close());
      return closing;
    },
  };
  for (const [method, operation] of Object.entries(methods)) {
    client[method] = async (params = {}) => {
      if (closing) throw clientError('closed', 'client_closed', 'Mara client is closed');
      const input = snapshot(params);
      return enqueue(() => {
        if (!capabilities.operations.includes(operation)) {
          throw clientError('unavailable', 'unsupported_operation', `Backend does not support ${operation}; inspect capabilities`);
        }
        return adapter.execute(operation, input);
      });
    };
  }
  return Object.freeze(client);
}
