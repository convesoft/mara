// The output-size limit of an evaluation in Soufflé's worker (DES-RULE-EXECUTION). Emscripten's
// in-memory filesystem bounds no file, so `limitWrites` replaces `FS.write` of one instance: a
// write that would take a file under `directory` past `bound` bytes calls `stop` and throws
// `Stopped` into the module, which ends the run as the system ends Soufflé's process natively.
// It relies on the pinned Emscripten SDK routing every write of the program through that
// property; the smoke run of scripts/build-souffle-wasm.sh checks that it does.

/** Thrown into the module where the system would end Soufflé's process natively. */
export class Stopped extends Error {}

/** Bounds each file that the instance whose filesystem is `FS` writes under `directory`. */
export function limitWrites(FS, directory, bound, stop) {
  const write = FS.write;
  FS.write = (stream, buffer, offset, length, position, canOwn) => {
    const at = position ?? stream.position;
    if (stream.path?.startsWith(directory) && at + length > bound) {
      stop();
      throw new Stopped();
    }
    return write(stream, buffer, offset, length, position, canOwn);
  };
}
