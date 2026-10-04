// @mara implements REQ-SHARED-TYPESCRIPT-CLIENT
// @mara implements DES-JAVASCRIPT-CLIENT-ADAPTERS
// Browser-safe facade: adapters own setup, transport and domain semantics.
import { snapshotJson } from './internal/json.mjs';
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
  try { return snapshotJson(value); }
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
  // Memory host extensions share the same input snapshots, queue and close boundary.
  const memoryMethods = {
    loadSnapshot: 'load_snapshot', workspaceState: 'workspace_state', workspaceSources: 'workspace_sources',
    exportChanges: 'export_changes', checkChangeSetPreconditions: 'check_change_set_preconditions',
  };
  const extensions = Object.fromEntries(Object.entries(memoryMethods)
    .filter(([, operation]) => capabilities.operations.includes(operation)));
  for (const [method, operation] of Object.entries({ ...methods, ...extensions })) {
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
