//! Explanations: one run of a program in Soufflé's provenance mode, whose outputs give the
//! outcome and whose console explains the rows that caused it (REQ-EXPLAIN-OUTCOMES and
//! DES-RULE-EXECUTION in `docs/rules/evaluation.mara.md`).
//!
//! The run starts as every evaluation does, in [`super::spawn_and_wait`], within the same
//! limits. Its standard input carries the commands of [`mara_engine::evaluation::explain`];
//! its standard output, read on a thread of its own up to [`REPLY_BYTES`], carries the JSON
//! replies. Neither can block the run past its time limit: the commands are written on another
//! thread, and the wait for a reply is the loop that stops the run at that limit.

use std::io::{self, Read as _, Write as _};
use std::path::Path;
use std::process::Child;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

use mara_engine::evaluation::explain::{EXIT, Explainer, Explanation, START, Target, Vocabulary};
use mara_engine::evaluation::{self, Exit, Fault, Limit, Rows};
use mara_engine::facts::Relation;
use mara_engine::work::WorkCounts;

use super::{Run, Started, bounded_read, host, private_dir, read_rows};

/// The most bytes of replies that one run may print; past it, the rows not yet explained
/// cannot be.
pub const REPLY_BYTES: usize = 16 << 20;

/// What a run in provenance mode gives: the rows of its outputs, which make the outcome as an
/// evaluation's do, and the explanation of each row that `select` chose.
#[derive(Clone, Debug)]
pub struct Explained {
    pub rows: Rows,
    pub targets: Vec<Target>,
    pub explanations: Vec<Explanation>,
    /// Whether the key decides the whole outcome: every row has the explanation, or the reason
    /// it has none, that the exchange gave it, and the run then ended as a successful run does
    /// ([`settled`]). A row left without one by the end of the run (the time limit, an abort, a
    /// stray or invalid reply, the bound on replies), or a run stopped at a limit or failed after
    /// the last reply, depends on the machine at the time.
    pub settled: bool,
}

// @mara implements REQ-EXPLAIN-OUTCOMES
/// Runs `run.program` once in provenance mode: when the evaluation ends, `select` chooses the
/// rows to explain from its outputs, and each is explained in turn. A fault of the evaluation
/// leaves it without rows, as [`super::evaluate`] reports it; the console failing after the
/// evaluation ended, at the time limit included, leaves the rows and the outcome as they are
/// and the rows not yet explained without an explanation. Counts one subprocess spawn and one
/// program run.
pub fn explain(
    run: &Run,
    select: &mut dyn FnMut(&Rows) -> Vec<Target>,
    work: &mut WorkCounts,
) -> Result<Explained, Fault> {
    // Declared before the directory, so that it is dropped after it.
    let _started = Started::begin()?;
    let dir = private_dir(run)?;
    let read = evaluation::read_relations(run.program, run.relations);
    let mut console = Provenance {
        dir: dir.path(),
        run,
        read: &read,
        select,
        phase: Phase::Starting,
        printed: Vec::new(),
        stderr_at_ready: 0,
    };
    let exit = super::spawn_and_wait(
        run.souffle,
        dir.path(),
        run.limits,
        Some(&mut console),
        work,
    )
    .map_err(|error| host("cannot run Soufflé", &error))?;
    console.finish(exit)
}

/// What a console receives from Soufflé's standard output.
pub(super) enum Reply {
    /// One complete JSON value.
    Json(serde_json::Value),
    /// Text that is no JSON value, or more than [`REPLY_BYTES`]: nothing after it can be read.
    Broken { text: Vec<u8>, reason: String },
}

/// Drives a run in provenance mode.
pub(super) trait Console {
    /// Takes one reply; gives the next command to send, or `None` to end the run.
    fn reply(&mut self, reply: Reply) -> Option<String>;
}

/// The exchange with one Soufflé process: a thread that writes the commands to its standard
/// input, and one that reads its standard output, at most [`REPLY_BYTES`].
pub(super) struct Exchange<'c> {
    console: &'c mut dyn Console,
    commands: Option<Sender<String>>,
    output: Receiver<Chunk>,
    buffer: Vec<u8>,
    /// Whether the console ended the exchange.
    ended: bool,
}

enum Chunk {
    Bytes(Vec<u8>),
    /// The standard output ended, or exceeded [`REPLY_BYTES`].
    End {
        overflow: bool,
    },
}

impl<'c> Exchange<'c> {
    /// Takes the child's standard input and output, and sends the commands that start the
    /// exchange. Fails when the system cannot start one of the threads; the caller then ends
    /// the child.
    pub(super) fn start(
        child: &mut Child,
        console: &'c mut dyn Console,
    ) -> io::Result<Exchange<'c>> {
        let cannot = |error: io::Error| {
            io::Error::new(
                error.kind(),
                format!("cannot start a thread to exchange with Soufflé: {error}"),
            )
        };
        let (commands, pending) = mpsc::channel::<String>();
        let (chunks, output) = mpsc::channel();
        if let Some(mut stdin) = child.stdin.take() {
            let writer = move || {
                for command in pending {
                    if stdin.write_all(command.as_bytes()).is_err() {
                        break;
                    }
                }
            };
            thread::Builder::new()
                .name("mara-souffle-in".to_owned())
                .spawn(writer)
                .map_err(cannot)?;
        }
        if let Some(mut stdout) = child.stdout.take() {
            let reader = move || {
                let mut total = 0;
                let mut buffer = vec![0; 64 << 10];
                loop {
                    let read = match stdout.read(&mut buffer) {
                        Ok(0) | Err(_) => break,
                        Ok(read) => read,
                    };
                    total += read;
                    // Past the bound, the rest is read and dropped, so that Soufflé is not
                    // stopped by a closed pipe but ends on `exit` or at the time limit.
                    if total > REPLY_BYTES {
                        if total - read <= REPLY_BYTES {
                            let _ = chunks.send(Chunk::End { overflow: true });
                        }
                        continue;
                    }
                    if chunks.send(Chunk::Bytes(buffer[..read].to_vec())).is_err() {
                        return;
                    }
                }
                if total <= REPLY_BYTES {
                    let _ = chunks.send(Chunk::End { overflow: false });
                }
            };
            thread::Builder::new()
                .name("mara-souffle-out".to_owned())
                .spawn(reader)
                .map_err(cannot)?;
        }
        let _ = commands.send(START.to_owned());
        Ok(Exchange {
            console,
            commands: Some(commands),
            output,
            buffer: Vec::new(),
            ended: false,
        })
    }

    /// Waits at most `wait` for output, and hands each complete reply to the console.
    pub(super) fn wait(&mut self, wait: Duration) {
        let chunk = match self.output.recv_timeout(wait) {
            Ok(chunk) => chunk,
            Err(RecvTimeoutError::Timeout) => return,
            // The thread ended: so did the output.
            Err(RecvTimeoutError::Disconnected) => {
                thread::sleep(wait);
                return;
            }
        };
        let ended = match chunk {
            Chunk::Bytes(bytes) => {
                self.buffer.extend(bytes);
                false
            }
            Chunk::End { overflow: false } => true,
            Chunk::End { overflow: true } => {
                let reason = format!("Soufflé's replies exceed {} MiB", REPLY_BYTES >> 20);
                let text = std::mem::take(&mut self.buffer);
                self.receive(Reply::Broken { text, reason });
                return;
            }
        };
        // Each reply ends with `}`: a buffer that does not is read again only once more came.
        if !ended && !self.buffer.trim_ascii_end().ends_with(b"}") {
            return;
        }
        while !self.ended {
            let mut values =
                serde_json::Deserializer::from_slice(&self.buffer).into_iter::<serde_json::Value>();
            match values.next() {
                Some(Ok(value)) => {
                    let used = values.byte_offset();
                    self.buffer.drain(..used);
                    self.receive(Reply::Json(value));
                }
                Some(Err(error)) if ended || !error.is_eof() => {
                    let text = std::mem::take(&mut self.buffer);
                    let reason = format!("Soufflé's reply is not valid JSON: {error}");
                    self.receive(Reply::Broken { text, reason });
                }
                // Only whitespace, or a reply not yet complete.
                _ => return,
            }
        }
    }

    fn receive(&mut self, reply: Reply) {
        if self.ended {
            return;
        }
        match self.console.reply(reply) {
            Some(command) => {
                if let Some(commands) = &self.commands {
                    let _ = commands.send(command);
                }
            }
            None => {
                self.ended = true;
                if let Some(commands) = self.commands.take() {
                    // Dropping the sender closes Soufflé's standard input after the command.
                    let _ = commands.send(EXIT.to_owned());
                }
            }
        }
    }
}

/// Where a run in provenance mode is.
enum Phase<'r> {
    /// Before the first reply: the evaluation runs.
    Starting,
    /// The evaluation ended with a fault, found when it answered.
    Failed(Fault),
    /// The evaluation ended with these rows; their explanations are being received.
    Explaining {
        rows: Rows,
        explainer: Box<Explainer<'r>>,
        /// Why the exchange broke off, when it did.
        broken: Option<String>,
    },
}

struct Provenance<'r, 's> {
    dir: &'r Path,
    run: &'r Run<'r>,
    /// The relations the wrapper reads: the facts and the supplied inputs of the run.
    read: &'r [&'r Relation],
    select: &'s mut dyn FnMut(&Rows) -> Vec<Target>,
    phase: Phase<'r>,
    /// What Soufflé printed before its first reply that is no reply.
    printed: Vec<u8>,
    /// How much it had printed on its standard error when it first answered.
    stderr_at_ready: usize,
}

impl Console for Provenance<'_, '_> {
    fn reply(&mut self, reply: Reply) -> Option<String> {
        match (&mut self.phase, reply) {
            (Phase::Starting, Reply::Json(_)) => {
                // Soufflé answers only once the evaluation ended and wrote its outputs.
                let stderr = bounded_read(&self.dir.join(evaluation::STDERR));
                self.stderr_at_ready = stderr.len();
                let finished = Exit {
                    success: true,
                    limit: None,
                };
                let limits = self.run.limits;
                let rows = evaluation::interpret(self.run.program, finished, &[], &stderr, limits)
                    .and_then(|()| read_rows(self.dir, self.run));
                match rows {
                    Err(fault) => {
                        self.phase = Phase::Failed(fault);
                        None
                    }
                    Ok(rows) => {
                        let targets = (self.select)(&rows);
                        let mut explainer = Box::new(Explainer::new(
                            Vocabulary::of(self.run.program),
                            self.read,
                            targets,
                        ));
                        let command = explainer.command();
                        self.phase = Phase::Explaining {
                            rows,
                            explainer,
                            broken: None,
                        };
                        command
                    }
                }
            }
            (Phase::Starting, Reply::Broken { text, .. }) => {
                self.printed = text;
                None
            }
            (Phase::Failed(_), _) => None,
            (Phase::Explaining { explainer, .. }, Reply::Json(value)) => {
                explainer.reply(&value);
                explainer.command()
            }
            (Phase::Explaining { broken, .. }, Reply::Broken { reason, .. }) => {
                *broken = Some(reason);
                None
            }
        }
    }
}

impl Provenance<'_, '_> {
    /// The rows and explanations once Soufflé ended as `exit` says, or the fault that left the
    /// evaluation without rows.
    fn finish(self, exit: Exit) -> Result<Explained, Fault> {
        let stderr = bounded_read(&self.dir.join(evaluation::STDERR));
        let limits = self.run.limits;
        match self.phase {
            Phase::Failed(fault) => Err(fault),
            Phase::Starting => {
                #[cfg(target_os = "linux")]
                if !exit.success
                    && let Some(problem) = super::sandbox::library_fault(self.run.souffle, &stderr)
                {
                    return Err(Fault::Host(problem));
                }
                evaluation::interpret(self.run.program, exit, &self.printed, &stderr, limits)?;
                Err(Fault::Runtime {
                    message: "Soufflé ended in provenance mode without answering".to_owned(),
                    at: None,
                })
            }
            Phase::Explaining {
                rows,
                explainer,
                broken,
            } => {
                let after =
                    String::from_utf8_lossy(&stderr[self.stderr_at_ready.min(stderr.len())..]);
                // How the run ended comes first: a reply cut short by an abort is no JSON.
                let aborted = aborted(exit, &after);
                let reason = match (exit.limit, aborted, broken) {
                    (Some(Limit::Time), _, _) => format!(
                        "the exchange with Soufflé passed the time limit of {} s \
                         (`[rules] time_limit_seconds`)",
                        limits.time_seconds
                    ),
                    (Some(Limit::Memory), _, _) => format!(
                        "provenance mode was stopped at the memory limit of {} MiB",
                        limits.memory_bytes >> 20
                    ),
                    (Some(Limit::Output), _, _) => format!(
                        "provenance mode was stopped at the output-size limit of {} MiB per file",
                        limits.output_bytes >> 20
                    ),
                    (None, true, _) => format!(
                        "provenance mode aborted: {}",
                        match after.trim() {
                            "" => "Soufflé failed without a message".to_owned(),
                            text => quote(text),
                        }
                    ),
                    (None, false, Some(reason)) => reason,
                    (None, false, None) => "Soufflé ended before it explained the row".to_owned(),
                };
                // A row explained before the run ended keeps its proof: the whole reply came and
                // was checked against the row.
                let targets = explainer.targets().to_vec();
                let settled = settled(explainer.settled(), exit, &after);
                Ok(Explained {
                    rows,
                    targets,
                    explanations: (*explainer).finish(&reason),
                    settled,
                })
            }
        }
    }
}

/// Whether a run that answered then failed: it exited unsuccessfully, or printed `after` on its
/// standard error after its first reply.
fn aborted(exit: Exit, after: &str) -> bool {
    !exit.success || !after.trim().is_empty()
}

/// Whether a run is settled ([`Explained::settled`]): every row it chose has the explanation
/// its exchange gave (`filled`, [`Explainer::settled`]), and it ended on its own, at no limit and
/// not [`aborted`], as a successful run does.
fn settled(filled: bool, exit: Exit, after: &str) -> bool {
    filled && exit.limit.is_none() && !aborted(exit, after)
}

/// At most [`evaluation::QUOTED_BYTES`] of `text`.
fn quote(text: &str) -> String {
    let mut end = text.len().min(evaluation::QUOTED_BYTES);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    match end < text.len() {
        true => format!("{}…", &text[..end]),
        false => text.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A run whose every row was explained is settled only when it then ended as a successful
    /// run does: not when the host stopped it at the deadline, nor when it exited unsuccessfully
    /// or printed on its standard error after the last reply.
    #[test]
    fn a_run_is_settled_only_when_every_row_was_explained_and_it_ended_successfully() {
        let ended = Exit {
            success: true,
            limit: None,
        };
        assert!(settled(true, ended, ""));
        assert!(settled(true, ended, "\n"), "nothing but whitespace");
        assert!(
            !settled(false, ended, ""),
            "a row left without its exchange"
        );
        let deadline = Exit {
            success: false,
            limit: Some(Limit::Time),
        };
        assert!(!settled(true, deadline, ""), "stopped at the deadline");
        let failed = Exit {
            success: false,
            limit: None,
        };
        assert!(!settled(true, failed, ""), "an unsuccessful exit");
        assert!(
            !settled(true, ended, "Error: aborted"),
            "an abort's message"
        );
    }
}
