import type { MaraClient, MemoryProjectInitParams } from './client.mjs';
export * from './client.mjs';
/** Complete effective UTF-8 inputs; the engine never discovers/fetches missing files. */
export interface SourceSnapshot {
  format_version: 1;
  files: { path: string; content: string }[];
  selection: {
    document_paths: string[];
    ignores: { directory: string; root_prefix: string[]; content: string }[];
  };
  /** Opaque provenance, not proof of current external/Git state. */
  base_identity?: string | null;
}
export interface WorkspaceState {
  format_version: 1;
  source_fingerprint: string;
  base_source_fingerprint: string;
  base_identity: string | null;
  mutation_persistence: 'memory';
}
export interface WorkspaceSources { base: SourceSnapshot; current: SourceSnapshot }
export interface FileChangeSet {
  format_version: 1;
  base_source_fingerprint: string;
  base_identity: string | null;
  /** Ordered normalized paths; null means absence/deletion, not an empty source. */
  changes: { path: string; before: string | null; after: string | null }[];
}
export interface ChangeSetPreconditionsParams { change_set: FileChangeSet; snapshot: SourceSnapshot }
export interface ChangeSetPreconditionsResult {
  format_version: 1;
  source_preconditions_match: true;
  external_context_verified: false;
}
export interface WasmClient extends MaraClient<MemoryProjectInitParams> {
  /** Successful replacement discards edits and establishes a new base. Failure preserves both. */
  loadSnapshot(snapshot: SourceSnapshot): Promise<WorkspaceState>;
  workspaceState(): Promise<WorkspaceState>;
  workspaceSources(): Promise<WorkspaceSources>;
  /** Consolidated delta only; never publishes, persists or advances the base. */
  exportChanges(): Promise<FileChangeSet>;
  /** Read-only whole-base/preimage check; external authorization/concurrency remain caller-owned. */
  checkChangeSetPreconditions(params: ChangeSetPreconditionsParams): Promise<ChangeSetPreconditionsResult>;
}
/** Owns a bundled module worker and a memory workspace. Required explicit snapshot, including empty initialization. */
export function createWasmClient(options: { snapshot: SourceSnapshot }): Promise<WasmClient>;
