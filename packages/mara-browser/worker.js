// The dedicated worker of a browser workspace (DES-BROWSER-WORKSPACE): it loads Mara's
// WebAssembly module, which runs every operation and hashes every file, and passes it the calls
// of the page's `Workspace` one at a time. The page posts `{ id, call, ... }`; the worker answers
// `{ id, answer }`, or `{ id, error }` when the module cannot be loaded or has stopped.
//
// A call and its answer are frames in the module's memory (ADR-RAW-WASM-MODULE): a JSON header's
// length as a 32-bit little-endian number, the header, then the bytes of the files it carries,
// which the header names by `at` and `length`.

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

function call(exports, header, files) {
  const text = encoder.encode(JSON.stringify(header));
  const length = files.reduce((sum, file) => sum + file.length, 4 + text.length);
  const at = exports.mara_alloc(length);
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
    exports.mara_answer(),
    exports.mara_answer_length(),
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
      let answer;
      try {
        answer = call(exports, header, files);
      } catch (error) {
        stopped = new Error(`Mara's WebAssembly module stopped: ${error.message}`);
        throw stopped;
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
