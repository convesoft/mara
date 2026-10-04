import type { MaraClient } from './client.mjs';
export * from './client.mjs';
export interface NativeOptions {
  /** Absolute caller context. Library setup ignores MARA environment defaults. */
  callerDirectory: string;
  /** Absolute, or relative to callerDirectory. Omit for nearest-project discovery. */
  project?: string;
  /** Default auto: fallback only for daemon absence/incompatibility before dispatch. */
  mode?: 'direct' | 'auto' | 'daemon-required';
  /** Absolute private IPC directory. No daemon is started by this client. */
  daemonDirectory?: string;
  /** Absolute local prebuilt addon override. Still requires exact application-version match. */
  addonPath?: string;
}
/** Requires Node 24+. Loads real N-API; all document methods are asynchronous. */
export function createNativeClient(options: NativeOptions): Promise<MaraClient>;
