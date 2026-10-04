export * from './client.mjs';
import type { MaraClient } from './client.mjs';
export interface HttpClientOptions {
  /** Deliberately handed-off numeric loopback http:// address ending in /rpc. No query or credentials. */
  endpoint: string;
  /** Daemon-session bearer credential. Not retained in public context or diagnostics. */
  token: string;
  /** Explicit absolute native project root or authorized initialization destination. */
  project: string;
}
/** Negotiates exact artifact/server versions. No discovery, replay, fallback or daemon administration.
 * All document methods are bound to project and reject per-call project overrides.
 * close drains earlier calls and releases only this client's resources.
 */
export function createHttpClient(options: HttpClientOptions): Promise<MaraClient>;
