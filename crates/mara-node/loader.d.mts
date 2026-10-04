/** Private addon boundary. Public per-operation result/parameter types are downstream. */
export interface NativeOptions {
  /** Absolute caller context; no process cwd/environment defaults are read. */
  callerDirectory: string;
  /** Relative to callerDirectory, or absolute. Omit for nearest-project discovery. */
  project?: string;
  mode?: 'direct' | 'auto' | 'daemon-required';
  /** Absolute private Unix-socket host directory. */
  daemonDirectory?: string;
}
export type NativeOperation =
  | 'capabilities' | 'project_init' | 'project_mid_backfill'
  | 'project_transaction_rollback' | 'project_refresh'
  | 'schema_get' | 'schema_list' | 'schema_validate' | 'project_validate'
  | 'item_validate' | 'item_list' | 'search' | 'get' | 'related'
  | 'item_create' | 'item_update' | 'item_move' | 'item_delete' | 'item_rename'
  | 'relation_get' | 'relation_add' | 'relation_remove' | 'trace_matrix';
export type NativeFailureKind = 'operation' | 'invalid_argument' | 'unavailable'
  | 'incompatible' | 'connection' | 'transport' | 'unknown_outcome' | 'closed';
export class NativeClientError extends Error {
  readonly kind: NativeFailureKind;
  readonly code: string;
  /** Complete owning error envelope, including relationship/unknown-outcome context. */
  readonly envelope: {
    format_version: number;
    error: { code: string; message: string; [key: string]: unknown };
    [key: string]: unknown;
  };
}
export interface AddonClient {
  /** Owning result object, not JSON-RPC. Invalid validation is a fulfilled result. */
  execute(operation: NativeOperation, params?: Record<string, unknown>): Promise<unknown>;
  /** Ordered, idempotent; drains preceding calls and releases only this handle. */
  close(): Promise<void>;
}
export function loadAddon(addonPath: string): (options: NativeOptions) => AddonClient;
