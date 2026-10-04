// @mara implements REQ-LOCAL-DAEMON-HTTP
// @mara implements REQ-SHARED-TYPESCRIPT-CLIENT
// @mara implements DES-JAVASCRIPT-CLIENT-ADAPTERS
import { createClient, MaraClientError, clientError } from './client.mjs';
import { applicationVersion } from './version.mjs';
export * from './client.mjs';

const operations = [
  'project_init', 'project_mid_backfill', 'project_transaction_rollback', 'project_refresh',
  'schema_get', 'schema_list', 'schema_validate', 'item_list', 'search', 'get', 'related',
  'item_create', 'item_update', 'item_delete', 'item_move', 'item_rename', 'item_validate',
  'project_validate', 'relation_get', 'relation_add', 'relation_remove', 'trace_matrix',
];
const mutations = new Set([
  'project_init', 'project_mid_backfill', 'project_transaction_rollback', 'project_refresh',
  'item_create', 'item_update', 'item_delete', 'item_move', 'item_rename',
  'relation_add', 'relation_remove',
]);
const formats = { discovery: 2, relationship: 1, validation: 1, trace: 1, operation_error: 1 };
const object = value => value !== null && typeof value === 'object' && !Array.isArray(value);

function setup(options) {
  if (!object(options) || Object.keys(options).some(key => !['endpoint', 'token', 'project'].includes(key))) {
    throw clientError('invalid_argument', 'invalid_argument', 'Supply named endpoint, token and project HTTP options');
  }
  const { endpoint, token, project } = options;
  let url;
  try { url = new URL(endpoint); } catch { /* handled below without echoing input */ }
  if (typeof endpoint !== 'string' || !/^http:\/\/(?:127(?:\.\d{1,3}){3}|\[::1\])(?::\d+)?\/rpc$/.test(endpoint)
      || !url || url.protocol !== 'http:' || url.pathname !== '/rpc' || url.search || url.hash
      || url.username || url.password) {
    throw clientError('invalid_argument', 'invalid_argument', 'Supply the numeric loopback HTTP /rpc endpoint from local connection-info, without credentials or query');
  }
  if (typeof token !== 'string' || !/^[\x21-\x7e]+$/.test(token)) {
    throw clientError('invalid_argument', 'invalid_argument', 'Supply the deliberately handed-off daemon-session token');
  }
  if (typeof project !== 'string' || !project.startsWith('/') || project.includes('\0')) {
    throw clientError('invalid_argument', 'invalid_argument', 'Supply an absolute native project root or initialization destination');
  }
  return { endpoint: url.href, token, project };
}

function negotiated(result) {
  const caps = result?.capabilities;
  if (!object(result) || result.protocol_version !== 1 || result.application_version !== applicationVersion
      || !object(result.result_formats) || Object.entries(formats).some(([key, version]) => result.result_formats[key] !== version)
      || !object(caps) || caps.backend !== 'native' || caps.persistence !== 'filesystem'
      || !Array.isArray(caps.operations) || caps.operations.length !== operations.length
      || new Set(caps.operations).size !== operations.length || operations.some(name => !caps.operations.includes(name))
      || caps.code_indexing !== true || caps.configured_indexers !== true || caps.transaction_rollback !== true
      || caps.force_refresh !== true || caps.project_sessions !== 'reconciled_memory'
      || !Array.isArray(caps.local_methods) || caps.local_methods.length !== 0) {
    throw clientError('incompatible', 'incompatible_capabilities', 'HTTP daemon must advertise the exact native protocol, result formats and project capabilities');
  }
  return {
    backend: 'native', application_version: result.application_version,
    result_formats: result.result_formats, operations: caps.operations,
    mutation_persistence: caps.persistence,
    code: { indexing: caps.code_indexing, configured_indexers: caps.configured_indexers },
    native_recovery: caps.transaction_rollback, force_refresh: caps.force_refresh,
  };
}

/** Browser fetch transport. Every attempt is single-dispatch; no reconnection/replay/fallback. */
export async function createHttpClient(options) {
  let { endpoint, token, project } = setup(options);
  if (typeof globalThis.fetch !== 'function') {
    throw clientError('unavailable', 'http_unavailable', 'This host does not provide browser fetch');
  }
  let id = 0;
  let closed = false;
  function uncertain(operation, message) {
    if (!mutations.has(operation)) return clientError('transport', 'http_transport_error', message);
    return new MaraClientError('unknown_outcome', { format_version: 1, error: {
      code: 'unknown_outcome', operation, project,
      message: `The dispatched ${operation} outcome is unknown. Do not retry or replay; inspect canonical source and recovery state`,
    } });
  }
  async function request(operation, params) {
    const requestId = ++id;
    const body = JSON.stringify({ jsonrpc: '2.0', id: requestId, method: operation, params });
    if (new TextEncoder().encode(body).length > 1024 * 1024) {
      throw clientError('invalid_argument', 'request_too_large', 'HTTP request exceeds 1 MiB; no operation was dispatched');
    }
    let response;
    try {
      response = await globalThis.fetch(endpoint, {
        method: 'POST', mode: 'cors', credentials: 'omit', redirect: 'error', cache: 'no-store',
        referrerPolicy: 'no-referrer',
        headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}`,
          'X-Mara-Protocol-Version': '1', 'X-Mara-Application-Version': applicationVersion },
        body,
      });
    } catch {
      if (mutations.has(operation)) throw uncertain(operation);
      throw clientError('connection', 'http_connection_failed', 'Could not reach the authorized HTTP daemon; check its availability and origin policy');
    }
    if (response.status === 401) throw clientError('authentication', 'authentication_failed', 'HTTP daemon refused the session credential; obtain a deliberate local handoff');
    if (response.status === 403) throw clientError('authorization', 'authorization_failed', 'HTTP daemon refused origin or exact project/destination authority');
    if ([400, 404, 405, 413].includes(response.status)) {
      throw clientError('invalid_argument', 'http_request_rejected', 'HTTP request was refused before dispatch; check the endpoint and protocol framing');
    }
    if (response.status === 503) throw clientError('unavailable', 'daemon_unavailable', 'HTTP daemon is stopping; no operation was dispatched');
    if (response.status !== 200) throw uncertain(operation, 'HTTP response did not establish the operation outcome');
    let rpc;
    try { rpc = await response.json(); }
    catch { throw uncertain(operation, 'HTTP response could not be decoded'); }
    // Parse/invalid-request refusals cannot echo an undecoded ID. They establish no dispatch.
    const unidentifiedRejection = object(rpc) && rpc.id === null && object(rpc.error)
      && [-32700, -32600].includes(rpc.error.code) && typeof rpc.error.message === 'string';
    if (!object(rpc) || rpc.jsonrpc !== '2.0' || (rpc.id !== requestId && !unidentifiedRejection)
        || Object.hasOwn(rpc, 'result') === Object.hasOwn(rpc, 'error')) {
      throw uncertain(operation, 'HTTP response did not match the request');
    }
    if (Object.hasOwn(rpc, 'result')) return rpc.result;
    const error = rpc.error;
    if (!object(error) || !Number.isInteger(error.code) || typeof error.message !== 'string') {
      throw uncertain(operation, 'HTTP error response was malformed');
    }
    const codes = {
      '-32700': ['invalid_argument', 'invalid_argument'], '-32600': ['invalid_argument', 'invalid_argument'],
      '-32601': ['unavailable', 'unsupported_operation'], '-32602': ['invalid_argument', 'invalid_argument'],
      '-32001': ['incompatible', 'negotiation_required'], '-32002': ['incompatible', 'incompatible_version'],
      '-32000': ['operation', 'operation_failed'],
    };
    const classified = codes[error.code];
    if (!classified) throw uncertain(operation, 'HTTP RPC failure did not establish the operation outcome');
    const [kind, code] = classified;
    const envelope = error.data;
    if (object(envelope) && envelope.format_version === 1 && object(envelope.error)
        && typeof envelope.error.code === 'string' && typeof envelope.error.message === 'string') {
      throw new MaraClientError(kind, envelope);
    }
    if (error.code === -32000) throw uncertain(operation, 'HTTP operation error was missing its owning envelope');
    throw clientError(kind, code, error.message);
  }
  return createClient({
    context: { backend: 'http', selection: { endpoint, project } },
    async execute(operation, params) {
      if (closed) throw clientError('closed', 'client_closed', 'Mara HTTP client is closed');
      if (operation === 'capabilities') return negotiated(await request('mara.negotiate', {
        protocol_version: 1, application_version: applicationVersion,
      }));
      if (!object(params) || Object.hasOwn(params, 'project')) {
        throw clientError('invalid_argument', 'invalid_argument', 'HTTP client is bound to its selected project; project overrides are not permitted');
      }
      return request(operation, { ...params, project });
    },
    async close() { closed = true; token = ''; endpoint = ''; project = ''; },
  });
}
