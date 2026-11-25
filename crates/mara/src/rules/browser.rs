//! Soufflé in a browser workspace (DES-RULE-EXECUTION in `docs/rules/evaluation.mara.md`): the
//! package's worker runs each evaluation in Soufflé's worker, where the module cannot wait for
//! it, so a request is answered by passes.
//!
//! A pass runs the request as the native host does, up to each evaluation. An evaluation whose
//! run the worker has not made yet records that run, the files of its private directory with
//! Soufflé's arguments, standard input and limits, and is a fault of the host for this pass,
//! which no session keeps: it keeps outcomes and compile errors only. After a pass that recorded runs, the module answers
//! them instead of the result; the worker makes them and supplies what each left ([`supply`]),
//! and the module runs the request again, until a pass records none. Its result is that last
//! pass's, read from the supplied runs exactly as the native host reads a child process's.
//!
//! A run in provenance mode is a console exchange, whose next command depends on Soufflé's last
//! reply. The worker cannot wait for the module, so each round runs Soufflé again with every
//! command so far and `exit`, and the console reads its replies from the start: Soufflé
//! evaluates a program alike each time. A round that wants another command asks for the next
//! round. A round stopped at the time limit printed nothing that the worker kept: the round
//! before it, which ended on `exit`, holds every reply but the one it waited for. So a run keeps
//! its last two rounds and drops older ones, whose outputs may be large.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::io;
use std::rc::Rc;

use mara_engine::evaluation::explain::{EXIT, START, Target};
use mara_engine::evaluation::{self, Exit, Fault, Limit, Limits, Rows};
use mara_engine::hash::Hash;
use mara_engine::work::WorkCounts;

use super::explain::{Console, Explained, Provenance, REPLY_BYTES, Replies, Reply};
use super::{Left, Run};

/// The name of Soufflé's module, which identifies it ([`super::souffle`]).
pub const MODULE: &str = "souffle.wasm";

/// The most bytes of what Soufflé prints on either stream that the module reads: 64 KiB, as
/// natively, and in provenance mode one more than [`REPLY_BYTES`] of its replies.
pub const PRINTED_BYTES: usize = 64 << 10;

/// A run for the worker to make.
#[derive(Clone, Debug)]
pub struct Pending {
    /// What identifies the run: its private directory's files, arguments and limits.
    pub id: Hash,
    /// Soufflé's arguments ([`super::arguments`]).
    pub arguments: Vec<&'static str>,
    /// The files of its private directory, by their path there; it also holds the empty
    /// directory [`evaluation::OUTPUT_DIR`].
    pub files: Vec<(String, Vec<u8>)>,
    /// What its standard input holds; `None` for an empty one.
    pub stdin: Option<Vec<u8>>,
    pub limits: Limits,
    /// The most bytes of its standard output that the module reads.
    pub stdout_bytes: usize,
}

/// What a run left, as the worker supplies it.
#[derive(Clone, Debug, Default)]
pub struct Ran {
    /// The exit status of `main`; `None` when the module stopped otherwise, such as by an abort
    /// or a trap, or was stopped at a limit.
    pub status: Option<i32>,
    /// The limit it was stopped at.
    pub limit: Option<Limit>,
    /// Why it could not run, such as a module that could not be loaded: a fault of the host.
    pub fault: Option<String>,
    /// What it printed on its standard output and error, up to the bounds of the run
    /// ([`Pending::stdout_bytes`], [`PRINTED_BYTES`]); `stdout_length` counts all of it.
    pub stdout: Vec<u8>,
    pub stdout_length: usize,
    pub stderr: Vec<u8>,
    /// How much it had printed on its standard error when it first printed on its standard
    /// output: in provenance mode, when it first answered.
    pub stderr_at_stdout: usize,
    /// The files it wrote under its private directory's output directory, by their path in the
    /// private directory.
    pub outputs: BTreeMap<String, Vec<u8>>,
}

/// A supplied run, with the commands its standard input held in provenance mode.
#[derive(Debug)]
struct Round {
    ran: Ran,
    commands: Vec<String>,
    /// Whether a pass counted it as a program run: each pass reads it again.
    counted: Cell<bool>,
}

/// How many supplied rounds a run keeps: the last, and the one before it for a last round
/// stopped at the time limit.
const KEPT_ROUNDS: usize = 2;

#[derive(Default)]
struct State {
    /// The runs this pass recorded, in order.
    pending: Vec<Pending>,
    /// The commands of each run recorded in provenance mode, for its supplied outcome.
    commands: HashMap<Hash, Vec<String>>,
    /// The supplied runs of the request by their identity, in order: in provenance mode one per
    /// round, at most [`KEPT_ROUNDS`].
    rounds: HashMap<Hash, VecDeque<Rc<Round>>>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::default();
}

/// The runs that the pass recorded, which the worker is to make; none when the pass gave the
/// request's result.
pub fn pending() -> Vec<Pending> {
    STATE.with(|state| std::mem::take(&mut state.borrow_mut().pending))
}

/// What the run `id` of the last pass left.
pub fn supply(id: Hash, ran: Ran) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let commands = state.commands.get(&id).cloned().unwrap_or_default();
        let round = Rc::new(Round {
            ran,
            commands,
            counted: Cell::new(false),
        });
        let rounds = state.rounds.entry(id).or_default();
        rounds.push_back(round);
        while rounds.len() > KEPT_ROUNDS {
            rounds.pop_front();
        }
    });
}

/// Forgets the runs of the request, once it has its result or another request starts.
pub fn end() {
    STATE.with(|state| *state.borrow_mut() = State::default());
}

/// The fault of an evaluation whose run the worker has yet to make: the pass's result is not
/// the request's, and no session keeps it.
fn waiting() -> Fault {
    Fault::Host("the evaluation waits for Soufflé's worker".to_owned())
}

/// The identity of a run of `run`: its mode, the project path of each file of its program, as
/// an evaluation's key holds them, so that two declared programs alike are two runs, its private
/// directory's files and its limits.
fn identity(provenance: bool, run: &Run, files: &[(String, Vec<u8>)]) -> Hash {
    let limits = run.limits;
    let mut bytes = Vec::new();
    let mut add = |part: &[u8]| {
        bytes.extend_from_slice(&(part.len() as u64).to_le_bytes());
        bytes.extend_from_slice(part);
    };
    add(&[u8::from(provenance)]);
    for path in &run.program.files {
        add(path.as_bytes());
    }
    for (name, contents) in files {
        add(name.as_bytes());
        add(contents);
    }
    add(&limits.time_seconds.to_le_bytes());
    add(&limits.memory_bytes.to_le_bytes());
    add(&limits.output_bytes.to_le_bytes());
    Hash::of_bytes(&bytes)
}

/// Records the run `id` of `files` for the worker, with `commands` on its standard input and
/// `exit` after them in provenance mode.
fn record(id: Hash, files: Vec<(String, Vec<u8>)>, commands: Option<Vec<String>>, limits: Limits) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        if state.pending.iter().any(|pending| pending.id == id) {
            return;
        }
        let provenance = commands.is_some();
        let stdin = commands.as_ref().map(|commands| {
            let mut stdin = commands.concat();
            stdin.push_str(EXIT);
            stdin.into_bytes()
        });
        if let Some(commands) = commands {
            state.commands.insert(id, commands);
        }
        state.pending.push(Pending {
            id,
            arguments: super::arguments(provenance),
            files,
            stdin,
            limits,
            stdout_bytes: match provenance {
                true => REPLY_BYTES + 1,
                false => PRINTED_BYTES,
            },
        });
    });
}

/// The kept rounds of the run `id`, oldest first, counting the last one as a program run unless
/// a pass already did: every run the worker made counts once.
fn rounds(id: Hash, work: &mut WorkCounts) -> Vec<Rc<Round>> {
    STATE.with(|state| {
        let state = state.borrow();
        let Some(rounds) = state.rounds.get(&id) else {
            return Vec::new();
        };
        if let Some(last) = rounds.back().filter(|last| !last.counted.get()) {
            last.counted.set(true);
            work.program_runs += 1;
        }
        rounds.iter().cloned().collect()
    })
}

/// The files of `run`'s private directory, owned.
fn files(run: &Run) -> Vec<(String, Vec<u8>)> {
    (super::private_files(run).into_iter())
        .map(|(name, bytes)| (name, bytes.into_owned()))
        .collect()
}

/// How a supplied run ended.
fn exit(ran: &Ran) -> Exit {
    Exit {
        success: ran.status == Some(0) && ran.limit.is_none(),
        limit: ran.limit,
    }
}

/// What a supplied run left, read as a private directory ([`Left`]); its standard error as far
/// as `stderr` shows it.
struct Supplied<'a> {
    ran: &'a Ran,
    stderr: Cell<usize>,
}

impl Left for Supplied<'_> {
    fn printed(&self, name: &str) -> Vec<u8> {
        let printed = match name {
            evaluation::STDERR => &self.ran.stderr[..self.stderr.get().min(self.ran.stderr.len())],
            evaluation::STDOUT => &self.ran.stdout[..],
            _ => &[],
        };
        printed[..printed.len().min(PRINTED_BYTES)].to_vec()
    }

    fn read(&self, path: &str) -> io::Result<Vec<u8>> {
        (self.ran.outputs.get(path).cloned())
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Soufflé wrote no such file"))
    }
}

// @mara implements REQ-PROJECT-AUTHORED-LOGIC
// @mara implements REQ-UNAVAILABLE-NOT-PARTIAL
/// The outcome of `run` in a browser workspace: read from the run that the worker made, as
/// [`super::evaluate`] reads a child process's, or recorded for the worker. Counts one program
/// run for the run made.
pub fn evaluate(run: &Run, work: &mut WorkCounts) -> Result<Rows, Fault> {
    let files = files(run);
    let id = identity(false, run, &files);
    let Some(round) = rounds(id, work).pop() else {
        record(id, files, None, run.limits);
        return Err(waiting());
    };
    let ran = &round.ran;
    if let Some(problem) = &ran.fault {
        return Err(Fault::Host(problem.clone()));
    }
    let left = Supplied {
        ran,
        stderr: Cell::new(ran.stderr.len()),
    };
    super::outcome(run, exit(ran), &left)
}

/// Where the console stopped reading a round's replies.
enum Stop {
    /// It ended the exchange, or the replies ended.
    Done,
    /// It wants a command that the round did not send.
    Wants(String),
    /// It wanted another command than the round sent: Soufflé answered differently.
    Diverged,
}

// @mara implements REQ-EXPLAIN-OUTCOMES
/// `run` in provenance mode in a browser workspace, explaining the rows that `select` chooses:
/// the console of [`super::explain`] reads the replies of the last round that the worker made,
/// and records the next round when it wants another command. Counts one program run per round
/// made.
pub fn explain(
    run: &Run,
    select: &mut dyn FnMut(&Rows) -> Vec<Target>,
    work: &mut WorkCounts,
) -> Result<Explained, Fault> {
    let files = files(run);
    let id = identity(true, run, &files);
    let rounds = rounds(id, work);
    let Some(last) = rounds.last() else {
        record(id, files, Some(vec![START.to_owned()]), run.limits);
        return Err(waiting());
    };
    if let Some(problem) = &last.ran.fault {
        return Err(Fault::Host(problem.clone()));
    }
    // A round stopped at the time limit printed nothing that was kept: the round before it
    // ended on `exit` after every reply but the one this round waited for.
    let stopped = last.ran.limit == Some(Limit::Time) && rounds.len() > 1;
    let round = match stopped {
        true => &rounds[rounds.len() - 2],
        false => last,
    };
    let ran = &round.ran;
    let left = Supplied {
        ran,
        stderr: Cell::new(ran.stderr_at_stdout),
    };
    let read = evaluation::read_relations(run.program, run.relations);
    let mut console = Provenance::new(&left, run, &read, select);
    let stop = replay(&mut console, round);
    left.stderr.set(ran.stderr.len());
    let ended = match stopped {
        true => Exit {
            success: false,
            limit: Some(Limit::Time),
        },
        false => exit(ran),
    };
    match stop {
        Stop::Diverged => Err(Fault::Host(
            "Soufflé answered differently in two runs of the same program in provenance mode"
                .to_owned(),
        )),
        // The round ended on `exit` after its last reply: the next round sends one more
        // command.
        Stop::Wants(command) if !stopped && ended.success => {
            let mut commands = round.commands.clone();
            commands.push(command);
            record(id, files, Some(commands), run.limits);
            Err(waiting())
        }
        Stop::Wants(_) | Stop::Done => console.finish(ended),
    }
}

/// Hands the console each reply of `round` as the native exchange does
/// ([`super::explain::Exchange`]), checking each command it gives against those the round sent.
fn replay(console: &mut dyn Console, round: &Round) -> Stop {
    let ran = &round.ran;
    // The first command, `START`, was sent before any reply.
    let mut sent = 1;
    let mut receive = |reply: Reply| match console.reply(reply) {
        None => Some(Stop::Done),
        Some(command) => match round.commands.get(sent) {
            Some(expected) if *expected == command => {
                sent += 1;
                None
            }
            Some(_) => Some(Stop::Diverged),
            None => Some(Stop::Wants(command)),
        },
    };
    let mut replies = Replies::default();
    let shown = &ran.stdout[..ran.stdout.len().min(REPLY_BYTES)];
    if replies.take(shown) {
        while let Some(reply) = replies.next(false) {
            if let Some(stop) = receive(reply) {
                return stop;
            }
        }
    }
    if ran.stdout_length > REPLY_BYTES {
        return receive(replies.overflow()).unwrap_or(Stop::Done);
    }
    while let Some(reply) = replies.next(true) {
        if let Some(stop) = receive(reply) {
            return stop;
        }
    }
    Stop::Done
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A run keeps only its last two rounds, however many were supplied, and counts the last one
    /// once.
    #[test]
    fn a_run_keeps_at_most_its_last_two_rounds() {
        end();
        let id = Hash::of_bytes(b"a run");
        for round in 0..5u8 {
            let ran = Ran {
                stdout: vec![round; 1 << 10],
                ..Ran::default()
            };
            supply(id, ran);
        }
        let kept = STATE.with(|state| state.borrow().rounds[&id].len());
        assert_eq!(kept, KEPT_ROUNDS);
        let mut work = WorkCounts::default();
        let rounds = rounds(id, &mut work);
        let firsts: Vec<u8> = rounds.iter().map(|round| round.ran.stdout[0]).collect();
        assert_eq!(firsts, [3, 4]);
        let _ = super::rounds(id, &mut work);
        assert_eq!(work.program_runs, 1);
        end();
    }
}
