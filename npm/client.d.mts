/** JSON owning data. Numbers use ordinary JS precision, never BigInt. */
export type JsonValue = null | boolean | number | string | JsonValue[] | { [key: string]: JsonValue };
export type JsonObject = { [key: string]: JsonValue };
export type FailureKind = 'operation' | 'invalid_argument' | 'unavailable' | 'incompatible'
  | 'connection' | 'transport' | 'unknown_outcome' | 'closed';
export interface ErrorEnvelope {
  format_version: 1;
  error: { code: string; message: string; operation?: string; project?: string; [key: string]: unknown };
  edge?: RelationEdge;
  occurrence_count?: number;
  [key: string]: unknown;
}
/** Unknown outcomes prohibit replay; retain the native source/recovery guidance. */
export class MaraClientError extends Error {
  constructor(kind: FailureKind, envelope: ErrorEnvelope);
  readonly kind: FailureKind;
  readonly code: string;
  readonly envelope: ErrorEnvelope;
}
export function clientError(kind: FailureKind, code: string, message: string): MaraClientError;
/** Adapter-owned setup identity, not a claim that project discovery resolved a root. */
export interface ClientContext { readonly backend: string; readonly selection: Readonly<JsonObject> }
export interface Capabilities {
  readonly backend?: string;
  readonly operations: readonly string[];
  readonly mutation_persistence: 'filesystem' | 'memory';
  readonly code: { readonly indexing: boolean; readonly configured_indexers?: boolean; readonly native_index_import?: boolean };
  readonly native_recovery: boolean;
  readonly force_refresh?: boolean;
  readonly application_version?: string;
  readonly result_formats?: Readonly<Record<string, number>>;
}
export interface ProjectSelection { /** Absolute per-call selection on unbound native handles only. */ project?: string | null }
/** Opaque cursor: repeat unchanged request until has_more=false; restart after inputs change. */
export interface PageParams { limit?: number | null; cursor?: string | null }
export interface Continuation { has_more: boolean; next_cursor: string | null }
export interface FieldValue { key: string; value: string }
export interface ItemFilterParams extends PageParams, ProjectSelection {
  flavours?: string[]; fields?: FieldValue[]; relations?: string[]; paths?: string[];
}
export interface SearchParams extends ItemFilterParams { query: string; ids?: string[] }
export interface GetParams extends ProjectSelection { reference: string; cursor?: string | null }
export interface RelatedParams extends GetParams, PageParams {
  direction?: 'incoming' | 'outgoing' | 'symmetric' | null; relations?: string[]; flavours?: string[];
}
export interface ItemCreateParams extends ProjectSelection {
  flavour: string; id: string; file: string; title: string;
  fields?: FieldValue[]; relations?: { relation: string; target: string }[];
  body?: string | null; line?: number | null;
}
export interface ItemUpdateParams extends ProjectSelection {
  reference: string; title?: string | null; fields?: FieldValue[]; clear_fields?: string[]; body?: string | null;
}
export interface ItemMoveParams extends ProjectSelection { reference: string; file: string; line?: number | null }
export interface RelationParams extends ProjectSelection { source: string; relation: string; target: string }
export interface RelationGetParams extends RelationParams, PageParams {}
export interface RelationRemoveParams extends RelationParams { occurrence?: string | null }
export interface TraceCheck { files: string[]; shape: string; parameters?: Record<string, string> }
export interface TraceSelection { ids?: string[]; flavours?: string[]; fields?: FieldValue[]; paths?: string[]; all?: boolean }
export interface TraceMatrixParams extends TraceSelection, PageParams, ProjectSelection {
  rules?: string[]; check?: TraceCheck | null; render?: string | null;
}
export type SchemaKind = 'flavour' | 'relation';
export interface SchemaGetParams extends ProjectSelection { kind?: SchemaKind | null; name?: string | null }
export type ValidationParams = PageParams & ProjectSelection;
export interface ProjectValidationParams extends ValidationParams { paths?: string[] }
export interface ItemValidationParams extends ValidationParams { id: string }
export type Template = 'minimal' | 'empty' | 'engineering';
export interface ProjectInitParams extends ProjectSelection { template?: Template }
/** Existing worker/WASM initialization; no native project override or filesystem selection. */
export interface MemoryProjectInitParams { name: string; template?: Template }
export interface ProjectSummary { root: string; name: string; schema_path: string; content_patterns: string[] }
export interface ProjectInitializationResult { project: ProjectSummary; created: string[] }
export interface BackfilledMid { id: string; mid: string; path: string; line: number }
export interface ProjectMidBackfillResult { project: string; changed: BackfilledMid[] }
export interface TransactionRollbackResult { project: string; restored: string[] }
export interface ItemIdentity { id: string; mid: string; path: string }
export interface ItemCreationResult extends ItemIdentity { line: number; complete: boolean; missing: string[] }
export interface ItemUpdateResult extends ItemIdentity {
  changed_fields: string[]; warnings: { scope: ValidationScope; path: string; line: number; message: string }[];
}
export interface ItemMoveResult { id: string; mid: string; old_location: ItemLocation; new_location: ItemLocation }
export interface ItemLocation { path: string; line: number }
export interface ItemRenameResult { mid: string; old_id: string; new_id: string; paths: string[] }
export interface ItemSummary { id: string; mid: string | null; flavour: string; title: string; path: string; line: number; title_truncated?: boolean }
export interface ItemCollectionResult extends Continuation { items: ItemSummary[] }
export interface SourceLocation { path: string; start_byte: number; end_byte: number; start_line: number; end_line: number }
export type MarkdownBlockKind = 'paragraph' | 'thematic_break' | 'code_block' | 'blockquote' | 'list' | 'list_item'
  | 'html_block' | 'link_reference_definition' | 'table' | 'table_header' | 'table_body' | 'table_row' | 'table_cell'
  | { heading: { level: number } };
export interface DiscoveryNode {
  reference: string; kind: 'item' | 'code' | 'section' | 'block' | 'document'; source: SourceLocation;
  title?: string; title_truncated: boolean; context: { parent?: string; section?: string };
  id?: string; mid?: string; flavour?: string; block_kind?: MarkdownBlockKind; heading_level?: number;
}
export interface SearchExcerpt { text: string; start_byte: number; end_byte: number; start_line: number; end_line: number; partial: boolean }
export interface SearchResult extends Continuation { format_version: 2; results: { node: DiscoveryNode; excerpt: SearchExcerpt }[] }
export interface TextRange { start_byte: number; end_byte: number; total_bytes: number; partial: boolean }
export interface EntryRange { start_index: number; end_index: number; total: number; partial: boolean }
export interface GetResult extends Continuation {
  format_version: 2; node: DiscoveryNode; content: string; content_range: TextRange;
  metadata: { index: number; key: string; value: string; range: TextRange }[]; metadata_range: EntryRange;
}
export type RelationEndpoint = { kind: 'item'; id: string; mid: string } | { kind: 'external'; address: string } | { kind: 'code'; reference: string };
export interface RelationEdge { relation: string; source: RelationEndpoint; target: RelationEndpoint; symmetric: boolean }
export interface RelatedResult extends Continuation {
  format_version: 2; node: DiscoveryNode;
  connections: { relation: string; direction: 'incoming' | 'outgoing' | 'symmetric';
    neighbour: DiscoveryNode | { kind: 'external'; address: string }; source?: SourceLocation;
    label?: string; edge?: RelationEdge; occurrence_count?: number }[];
}
export interface RelationInspection extends Continuation {
  format_version: 1; edge: RelationEdge; occurrence_count: number;
  occurrences: { reference: string; kind: string; source: SourceLocation; author: RelationEndpoint; relation: string; target: string }[];
}
export interface RelationMutationResult {
  format_version: 1; action: 'added' | 'removed'; scope: string; edge: RelationEdge;
  changed_occurrences: number; remaining_occurrences: number; edge_exists: boolean;
}
export type Severity = 'error' | 'warning';
export interface FieldDefinition { type: 'string' | 'integer' | 'number' | 'boolean' | 'enum'; required: boolean; repeatable: boolean; values?: string[] }
export interface FlavourDefinition {
  description: string; use_when: string[]; avoid_when: string[]; distinguish_from: Record<string, string>;
  id_prefix: string; body: 'optional' | 'required'; fields: Record<string, FieldDefinition>;
}
export interface CardinalityBounds { minimum?: number; maximum?: number; severity: Severity }
export interface RelationDefinition {
  description: string; source: string[]; target: string[]; same_flavour: boolean; inverse?: string; symmetric: boolean;
  external?: boolean; code_source?: boolean;
  cardinality?: { incoming?: CardinalityBounds; outgoing?: CardinalityBounds; symmetric?: CardinalityBounds };
  acyclic?: { severity: Severity };
}
export interface Schema { format_version: 3; flavours: Record<string, FlavourDefinition>; relations: Record<string, RelationDefinition> }
export type SchemaGetResult = { kind: 'schema'; schema: Schema }
  | { kind: 'flavour'; name: string; definition: FlavourDefinition }
  | { kind: 'relation'; name: string; requested_name: string; inverse: boolean; definition: RelationDefinition };
export interface SchemaListResult { kind: SchemaKind; declarations: { name: string; description: string; inverse?: string; symmetric?: boolean }[] }
export type ValidationScope = 'project' | 'schema' | 'document' | 'item';
export type DiagnosticCode = 'project_invalid' | 'schema_invalid' | 'format_unsupported' | 'source_invalid'
  | 'identity_invalid' | 'field_invalid' | 'reference_unresolved' | 'relation_invalid' | 'code_missing'
  | 'code_ambiguous' | 'code_unsupported' | 'rule_invalid' | 'rule_failed' | 'relation_cardinality' | 'relation_cycle' | 'evaluation_unavailable';
export interface DiagnosticLocation { path?: string; line?: number; start_byte?: number; end_byte?: number; pointer?: string }
export interface ValidationDiagnostic {
  code: DiagnosticCode; severity: Severity; scope: ValidationScope; message: string; location: DiagnosticLocation;
  path?: string; line?: number; item?: { id: string; mid?: string }; rule?: string;
  obligation?: { component: string; shape: string; source: DiagnosticLocation }; details?: JsonValue;
}
/** valid:false (including incomplete evaluation) is fulfilled data, not an exception. */
export interface ValidationResult extends Continuation {
  format_version: 1; project: string; target: { kind: 'project' | 'item' | 'schema'; id?: string };
  valid: boolean; evaluation_complete: boolean; diagnostics: ValidationDiagnostic[];
  summary: { counts_exact: boolean; errors: number; warnings: number };
  selection: { paths: string[]; omitted_diagnostics: number } | null;
  path?: string; flavours?: number | null; relations?: number | null;
}
/** Owning rule observations are extensible JSON, not substituted domain objects. */
export type TraceRecord = { kind: 'result'; root: JsonValue; evaluation: JsonValue; state: string }
  | { kind: 'check'; reference: string; root: JsonValue; evaluation: JsonValue; obligation: JsonValue;
      context: JsonValue[]; parent: string | null; state: string; condition: JsonValue; counts: JsonValue;
      every: boolean | null; inspection: JsonValue; reported: JsonValue }
  | { kind: 'edge'; check: string; edge: JsonValue; label: string; direction: string; endpoint: JsonValue;
      outside_selection: boolean; qualification: string | null; every: string | null; occurrence_count: number; inspection: JsonValue }
  | { kind: 'issue'; diagnostic: JsonValue };
export interface TraceMatrixResult extends Continuation {
  format_version: 1; kind: string; evaluation_complete: boolean; selection: Required<TraceSelection>;
  summaries: { evaluation: JsonValue; counts_exact: boolean; selected: number; not_applicable: number; passed: number; failed: number; unavailable: number }[];
  records: TraceRecord[]; markdown?: string;
}
export interface OperationInputs<InitParams extends object = ProjectInitParams> {
  capabilities: ProjectSelection; project_init: InitParams;
  project_mid_backfill: ProjectSelection; project_transaction_rollback: ProjectSelection; project_refresh: ProjectSelection;
  schema_get: SchemaGetParams; schema_list: ProjectSelection & { kind: SchemaKind }; schema_validate: ValidationParams;
  project_validate: ProjectValidationParams; item_validate: ItemValidationParams; item_list: ItemFilterParams;
  search: SearchParams; get: GetParams; related: RelatedParams; item_create: ItemCreateParams; item_update: ItemUpdateParams;
  item_move: ItemMoveParams; item_delete: ProjectSelection & { reference: string }; item_rename: ProjectSelection & { reference: string; new_id: string };
  relation_get: RelationGetParams; relation_add: RelationParams; relation_remove: RelationRemoveParams; trace_matrix: TraceMatrixParams;
}
export interface OperationResults {
  capabilities: Capabilities; project_init: ProjectInitializationResult; project_mid_backfill: ProjectMidBackfillResult;
  project_transaction_rollback: TransactionRollbackResult; project_refresh: { refreshed: true };
  schema_get: SchemaGetResult; schema_list: SchemaListResult; schema_validate: ValidationResult;
  project_validate: ValidationResult; item_validate: ValidationResult; item_list: ItemCollectionResult;
  search: SearchResult; get: GetResult; related: RelatedResult; item_create: ItemCreationResult; item_update: ItemUpdateResult;
  item_move: ItemMoveResult; item_delete: ItemIdentity; item_rename: ItemRenameResult;
  relation_get: RelationInspection; relation_add: RelationMutationResult; relation_remove: RelationMutationResult; trace_matrix: TraceMatrixResult;
}
export type DocumentOperation = keyof OperationInputs;
/** Adapter implements owning formats/errors; setup and external publication stay adapter/application owned. */
export interface ClientAdapter<InitParams extends object = ProjectInitParams> {
  readonly context: ClientContext;
  execute(operation: DocumentOperation, params: OperationInputs<InitParams>[DocumentOperation]): Promise<unknown>;
  close(): Promise<void>;
}
export interface MaraClient<InitParams extends object = ProjectInitParams> {
  readonly context: ClientContext;
  readonly capabilities: Capabilities;
  /** Drains prior calls, rejects later calls, releases only this adapter; never stops a shared daemon. */
  close(): Promise<void>;
  projectInit(...args: {} extends InitParams ? [params?: InitParams] : [params: InitParams]): Promise<ProjectInitializationResult>;
  projectMidBackfill(params?: ProjectSelection): Promise<ProjectMidBackfillResult>;
  projectTransactionRollback(params?: ProjectSelection): Promise<TransactionRollbackResult>;
  projectRefresh(params?: ProjectSelection): Promise<{ refreshed: true }>;
  schemaGet(params?: SchemaGetParams): Promise<SchemaGetResult>;
  schemaList(params: OperationInputs['schema_list']): Promise<SchemaListResult>;
  schemaValidate(params?: ValidationParams): Promise<ValidationResult>;
  projectValidate(params?: ProjectValidationParams): Promise<ValidationResult>;
  itemValidate(params: ItemValidationParams): Promise<ValidationResult>;
  itemList(params?: ItemFilterParams): Promise<ItemCollectionResult>;
  search(params: SearchParams): Promise<SearchResult>;
  get(params: GetParams): Promise<GetResult>;
  related(params: RelatedParams): Promise<RelatedResult>;
  itemCreate(params: ItemCreateParams): Promise<ItemCreationResult>;
  itemUpdate(params: ItemUpdateParams): Promise<ItemUpdateResult>;
  itemMove(params: ItemMoveParams): Promise<ItemMoveResult>;
  itemDelete(params: ProjectSelection & { reference: string }): Promise<ItemIdentity>;
  itemRename(params: OperationInputs['item_rename']): Promise<ItemRenameResult>;
  relationGet(params: RelationGetParams): Promise<RelationInspection>;
  relationAdd(params: RelationParams): Promise<RelationMutationResult>;
  relationRemove(params: RelationRemoveParams): Promise<RelationMutationResult>;
  traceMatrix(params?: TraceMatrixParams): Promise<TraceMatrixResult>;
}
/** Async setup obtains capabilities before returning. Adapter ownership transfers even if setup fails. */
export function createClient<InitParams extends object = ProjectInitParams>(adapter: ClientAdapter<InitParams>): Promise<MaraClient<InitParams>>;
