// The dedicated worker of a browser workspace (DES-BROWSER-WORKSPACE): it loads Mara's
// WebAssembly module, which runs every operation and hashes every file, and passes it the calls
// of the page's `Workspace` one at a time. The page posts `{ id, call, ... }`; the worker answers
// `{ id, answer }`, or `{ id, error }` when the module cannot be loaded or has stopped.
//
// A call and its answer are frames in the module's memory (ADR-RAW-WASM-MODULE): a JSON header's
// length as a 32-bit little-endian number, the header, then the bytes of the files it carries,
// which the header names by `at` and `length`.
//
// A request that evaluates programs is answered by passes (DES-RULE-EXECUTION): the module
// answers the runs of Soufflé it waits for, this worker makes each in Soufflé's worker, one at a
// time, and supplies what they left with the call `ran`, until the module answers the result.
// Soufflé's worker holds nothing of the workspace: at a run's time limit this worker terminates
// it and starts another for the next run.

const moduleUrl = new URL("./mara.wasm", import.meta.url);
const encoder = new TextEncoder();
const decoder = new TextDecoder();

let memory;
const imports = {
  mara: {
    now: () => Date.now(),
    random: (at, length) => {
      crypto.getRandomValues(new Uint8Array(memory.buffer, at, length));
    },
  },
};

const loaded = (async () => {
  let response;
  try {
    response = await fetch(moduleUrl);
  } catch (error) {
    throw new Error(`cannot load Mara's WebAssembly module ${moduleUrl}: ${error.message}`);
  }
  if (!response.ok) {
    throw new Error(
      `cannot load Mara's WebAssembly module ${moduleUrl}: HTTP status ${response.status}`,
    );
  }
  const bytes = await response.arrayBuffer();
  try {
    const { instance } = await WebAssembly.instantiate(bytes, imports);
    memory = instance.exports.memory;
    return instance.exports;
  } catch (error) {
    throw new Error(`cannot load Mara's WebAssembly module ${moduleUrl}: ${error.message}`);
  }
})();
// A failed load is reported to each call.
loaded.catch(() => {});

// Set once the module trapped: its memory may be inconsistent, so it answers no other call.
let stopped;

const souffleWorkerUrl = new URL("./souffle-worker.js", import.meta.url);

// Soufflé's worker, `{ worker, loaded }`, started when a run needs it; `loaded` settles with
// `{ ready }` or `{ failed }`.
let souffle;

function startSouffle() {
  const worker = new Worker(souffleWorkerUrl, { type: "module" });
  const loaded = new Promise((resolve) => {
    worker.onmessage = ({ data }) => resolve(data);
    worker.onerror = (event) => {
      event.preventDefault();
      resolve({ failed: `Soufflé's worker ${souffleWorkerUrl} failed: ${event.message}` });
    };
  });
  return { worker, loaded };
}

function endSouffle() {
  souffle?.worker.terminate();
  souffle = undefined;
}

// Makes `run`, one of the runs the module waits for, with its files' bytes in `bytes`, in
// Soufflé's worker; resolves with what it left. The time limit counts from here, so that a worker
// that cannot load its module in time is terminated too. A worker that cannot be started, a
// module that could not be loaded and a worker that failed are each a fault of the run; another
// run starts another worker.
function make(run, bytes) {
  const part = ({ at, length }) => bytes.slice(at, at + length);
  return new Promise((resolve) => {
    let settled = false;
    let timer;
    const settle = (made) => {
      if (!settled) {
        settled = true;
        clearTimeout(timer);
        resolve(made);
      }
    };
    const fault = (message) => {
      if (!settled) {
        endSouffle();
        settle({ id: run.id, fault: message });
      }
    };
    timer = setTimeout(() => {
      endSouffle();
      settle({ id: run.id, limit: "time" });
    }, run.time_limit_ms);
    try {
      souffle ??= startSouffle();
    } catch (error) {
      fault(`cannot start Soufflé's worker ${souffleWorkerUrl}: ${error.message}`);
      return;
    }
    const { worker, loaded } = souffle;
    loaded.then((state) => {
      if (settled) {
        return;
      }
      if (state.failed) {
        fault(state.failed);
        return;
      }
      worker.onmessage = ({ data }) => {
        settle(data.fault ? { id: run.id, fault: data.fault } : { id: run.id, ...data.ran });
      };
      worker.onerror = (event) => {
        event.preventDefault();
        fault(`Soufflé's worker failed: ${event.message}`);
      };
      try {
        worker.postMessage({
          arguments: run.arguments,
          files: run.files.map((file) => ({ path: file.path, contents: part(file) })),
          stdin: run.stdin === null ? null : part(run.stdin),
          output_bytes: run.output_bytes,
          stdout_bytes: run.stdout_bytes,
          stderr_bytes: run.stderr_bytes,
        });
      } catch (error) {
        fault(`cannot send the run to Soufflé's worker: ${error.message}`);
      }
    });
  });
}

// The call `ran` that supplies what each of `made` left.
function ran(made) {
  const files = [];
  let at = 0;
  const place = (bytes) => {
    if (bytes === undefined) {
      return null;
    }
    files.push(bytes);
    const placed = { at, length: bytes.length };
    at += bytes.length;
    return placed;
  };
  const runs = made.map((run) => ({
    id: run.id,
    status: run.status ?? null,
    limit: run.limit ?? null,
    fault: run.fault ?? null,
    stdout: place(run.stdout),
    stdout_length: run.stdout_length ?? 0,
    stderr: place(run.stderr),
    stderr_at_stdout: run.stderr_at_stdout ?? 0,
    outputs: (run.outputs ?? []).map(({ path, contents }) => ({ path, ...place(contents) })),
  }));
  return { text: encoder.encode(JSON.stringify({ call: "ran", runs })), files };
}

// A refused call that the module never saw, in the form of the module's own refusals.
function refused(message) {
  return {
    failed: true,
    status: 2,
    reply: { format_version: 1, error: { code: "invalid-argument", message, diagnostics: [] } },
  };
}

// Runs one call of the header's JSON text `text` and `files`. The module's pointers and lengths
// are unsigned 32-bit numbers, which JavaScript receives as signed ones.
function call(exports, text, files) {
  const length = files.reduce((sum, file) => sum + file.length, 4 + text.length);
  const at = exports.mara_alloc(length) >>> 0;
  const frame = new Uint8Array(exports.memory.buffer, at, length);
  new DataView(frame.buffer, at, 4).setUint32(0, text.length, true);
  frame.set(text, 4);
  let offset = 4 + text.length;
  for (const file of files) {
    frame.set(file, offset);
    offset += file.length;
  }
  exports.mara_call(at, length);
  const answer = new Uint8Array(
    exports.memory.buffer,
    exports.mara_answer() >>> 0,
    exports.mara_answer_length() >>> 0,
  ).slice();
  const headerLength = new DataView(answer.buffer).getUint32(0, true);
  return {
    header: JSON.parse(decoder.decode(answer.subarray(4, 4 + headerLength))),
    files: answer.subarray(4 + headerLength),
  };
}

// The header and files of a page's call.
function request(message) {
  const files = [];
  let at = 0;
  const place = (bytes) => {
    files.push(bytes);
    const placed = { at, length: bytes.length };
    at += bytes.length;
    return placed;
  };
  switch (message.call) {
    case "create":
      return {
        header: {
          call: "create",
          files: message.files.map(({ path, contents }) => ({ path, ...place(contents) })),
        },
        files,
      };
    case "write":
      return {
        header: { call: "write", path: message.path, ...place(message.contents) },
        files,
      };
    default:
      return { header: message, files };
  }
}

// The page's answer: an export's changes carry their contents.
function reply(answer) {
  const { header, files } = answer;
  if (header.changes) {
    header.changes = header.changes.map(({ path, base, at, length }) => ({
      path,
      base,
      contents: at === null ? null : files.slice(at, at + length),
    }));
  }
  return header;
}

let queue = Promise.resolve();
self.onmessage = ({ data }) => {
  queue = queue.then(async () => {
    const { id, ...message } = data;
    try {
      const exports = await loaded;
      if (stopped) {
        throw stopped;
      }
      const { header, files } = request(message);
      // A header that JSON cannot hold, such as one with a BigInt or a cycle, never reaches the
      // module, which goes on.
      let text;
      try {
        text = encoder.encode(JSON.stringify(header));
      } catch (error) {
        self.postMessage({ id, answer: refused(`the call cannot be sent: ${error.message}`) });
        return;
      }
      const guarded = (text, files) => {
        try {
          return call(exports, text, files);
        } catch (error) {
          stopped = new Error(`Mara's WebAssembly module stopped: ${error.message}`);
          throw stopped;
        }
      };
      let answer = guarded(text, files);
      while (answer.header.pending) {
        const made = [];
        for (const run of answer.header.pending) {
          made.push(await make(run, answer.files));
        }
        const next = ran(made);
        answer = guarded(next.text, next.files);
      }
      const answered = reply(answer);
      const transfer = (answered.changes ?? [])
        .filter((change) => change.contents !== null)
        .map((change) => change.contents.buffer);
      self.postMessage({ id, answer: answered }, transfer);
    } catch (error) {
      self.postMessage({ id, error: String(error.message ?? error) });
    }
  });
};
