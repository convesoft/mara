// @mara implements REQ-NODE-NATIVE-BINDING
// @mara implements DES-JAVASCRIPT-CLIENT-ADAPTERS
import { createRequire } from 'node:module';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { loadAddon, NativeClientError } from './native/loader.mjs';
import { createClient, MaraClientError, clientError } from './client.mjs';
export * from './client.mjs';

const packages = new Map([
  ['linux:x64', '@convesoft/mara-linux-x64-gnu'],
  ['linux:arm64', '@convesoft/mara-linux-arm64-gnu'],
  ['darwin:arm64', '@convesoft/mara-darwin-arm64'],
]);

/** ESM-only native setup. No CLI subprocess, environment routing or daemon startup. */
export async function createNativeClient(options) {
  if (Number(process.versions.node.split('.')[0]) < 24) {
    throw clientError('incompatible', 'node_version_unsupported', 'Mara requires Node 24+');
  }
  if (!options || typeof options !== 'object' || Array.isArray(options)) {
    throw clientError('invalid_argument', 'invalid_argument', 'Supply named native setup options');
  }
  const { addonPath, ...nativeOptions } = options;
  // The generated manifest is the single workspace-version authority in the JS artifact.
  let manifest;
  try { manifest = JSON.parse(await readFile(new URL('./package.json', import.meta.url), 'utf8')); }
  catch { throw clientError('unavailable', 'native_package_invalid', 'Mara package manifest is missing or invalid; reinstall the package'); }
  let selected = addonPath;
  if (selected === undefined) {
    const packageName = packages.get(`${process.platform}:${process.arch}`);
    if (!packageName) throw clientError('unavailable', 'native_host_unsupported',
      'Supported native hosts are glibc Linux x64/arm64 and Apple Silicon macOS');
    const require = createRequire(import.meta.url);
    let packageManifest;
    try { packageManifest = require.resolve(`${packageName}/package.json`); }
    catch { throw clientError('unavailable', 'native_addon_missing',
      `Install matching ${packageName} with optional dependencies enabled`); }
    let platform;
    try { platform = JSON.parse(await readFile(packageManifest, 'utf8')); }
    catch { throw clientError('unavailable', 'native_package_invalid', 'Native platform package manifest is invalid; reinstall the matching package'); }
    if (platform.version !== manifest.version) throw clientError('incompatible', 'native_version_mismatch',
      'Native platform package must match the Mara package version exactly');
    selected = path.join(path.dirname(packageManifest), 'bin', 'mara.node');
  }
  if (typeof selected !== 'string' || !path.isAbsolute(selected)) {
    throw clientError('invalid_argument', 'invalid_argument', 'Supply an absolute addonPath');
  }
  let construct;
  try { construct = loadAddon(selected); }
  catch (error) {
    throw clientError('unavailable', 'native_addon_load_failed', `Could not load native addon: ${error.message}`);
  }
  let handle;
  try { handle = construct(nativeOptions); }
  catch (error) {
    if (error instanceof NativeClientError) throw new MaraClientError(error.kind, error.envelope);
    if (error.code === 'InvalidArg') {
      throw clientError('invalid_argument', 'invalid_argument', 'Supply JSON-compatible native setup options');
    }
    throw clientError('unavailable', 'native_addon_load_failed', `Could not initialize native addon: ${error.message}`);
  }
  const adapter = {
    context: { backend: 'native', selection: { ...nativeOptions, mode: nativeOptions.mode ?? 'auto' } },
    async execute(operation, params) {
      try {
        const result = await handle.execute(operation, params);
        if (operation === 'capabilities' && result.application_version !== manifest.version) {
          throw clientError('incompatible', 'native_version_mismatch', 'Native addon must match the Mara package version exactly');
        }
        return result;
      } catch (error) {
        if (error instanceof NativeClientError) throw new MaraClientError(error.kind, error.envelope);
        if (error instanceof MaraClientError) throw error;
        // No automatic replay. Conversion/task failures cannot establish mutation outcome.
        throw clientError('transport', 'native_transport_error',
          'Native addon response failed; do not replay mutations. Inspect canonical source and recovery state');
      }
    },
    async close() {
      try { await handle.close(); }
      catch (error) {
        if (error instanceof NativeClientError) throw new MaraClientError(error.kind, error.envelope);
        throw clientError('transport', 'native_transport_error', 'Native client resource release failed');
      }
    },
  };
  return createClient(adapter);
}
