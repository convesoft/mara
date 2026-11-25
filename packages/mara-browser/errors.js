// The failure of a request in the browser package: the workspace's and the daemon client's.

/** A failed request, with the failure object that `--format json` prints. */
export class MaraError extends Error {
  constructor(reply, stats) {
    super(reply.error.message);
    this.name = "MaraError";
    this.code = reply.error.code;
    this.diagnostics = reply.error.diagnostics;
    this.reply = reply;
    if (stats !== undefined) {
      this.stats = stats;
    }
  }
}
