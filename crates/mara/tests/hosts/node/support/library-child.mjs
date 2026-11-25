// A Node process that holds one client of the Node library (VER-NODE-LIBRARY): it imports the
// library by its name through the module of the test given as its first argument, creates a
// client with the options given as JSON in its second, and runs each call its parent sends. A
// call marked `timed` starts a 10 ms interval timer just before it and answers with the times of
// the call, of each callback and of the settling, by Node's monotonic clock.

const [library, options] = process.argv.slice(2);
const { createClient, MaraError } = await import(library);
const client = createClient(JSON.parse(options));

process.on("message", async ({ id, name, args, options, timed }) => {
  let promise;
  let threw;
  const callbacks = [];
  const timer = timed ? setInterval(() => callbacks.push(performance.now()), 10) : undefined;
  const called = performance.now();
  try {
    promise = client.run(name, args ?? undefined, options ?? undefined);
  } catch (error) {
    threw = String(error);
  }
  const promised = promise instanceof Promise && threw === undefined;
  const times = () => {
    clearInterval(timer);
    return timed ? { called, callbacks, settled: performance.now() } : undefined;
  };
  try {
    const result = await promise;
    process.send({ id, promised, result, times: times() });
  } catch (error) {
    const { code, message, diagnostics, reply, stats } = error;
    process.send({
      id,
      promised,
      times: times(),
      error: { maraError: error instanceof MaraError, code, message, diagnostics, reply, stats, threw },
    });
  }
});
