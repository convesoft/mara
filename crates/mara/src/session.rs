//! A daemon's sessions (DES-DAEMON-SESSIONS and DES-IDLE-SESSIONS in `docs/daemon/index.mara.md`):
//! one per canonical project root, holding that project's inputs between requests.
//!
//! A request runs against a view of its session's inputs ([`Inputs::shared`]): every request
//! checks the inputs on disk, so its answer reflects the files as they are when it is served,
//! whatever the watcher has delivered (REQ-NEVER-STALE). The watcher ([`crate::watch`]) only
//! makes that check cheap, by re-deriving in the background after a change ([`Session::derive`]).
//! The work a session does, in requests and in the background, is reported by its next
//! response ([`Session::take_work`], DES-PERFORMANCE-HARNESS). A session also keeps the outcomes
//! of its evaluations by key ([`Evaluated`]), so a request finds what an earlier one ran.

use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use mara_engine::evaluation::explain::{Explanation, Node, Target};
use mara_engine::evaluation::{Fault, Rows};
use mara_engine::facts::Value;
use mara_engine::hash::Hash;
use mara_engine::work::WorkCounts;

use crate::hooks::{self, Hook};
use crate::inputs::Inputs;
use crate::rules::{self, Explained, Run};

/// One project's session.
#[derive(Debug)]
pub struct Session {
    root: PathBuf,
    inputs: Arc<Mutex<Inputs>>,
    evaluated: Arc<Evaluated>,
    /// The work done since the previous response.
    work: Mutex<WorkCounts>,
    activity: Mutex<Activity>,
    deriving: Arc<Deriving>,
}

#[derive(Debug)]
struct Activity {
    /// When the last request began or ended, or when the session opened.
    last: Instant,
    /// Requests being served.
    active: usize,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Session {
    fn new(root: PathBuf) -> Session {
        Session {
            inputs: Arc::new(Mutex::new(Inputs::new(root.clone()))),
            evaluated: Arc::default(),
            root,
            work: Mutex::default(),
            activity: Mutex::new(Activity {
                last: Instant::now(),
                active: 0,
            }),
            deriving: Arc::default(),
        }
    }

    /// The canonical project root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// A view of the session's inputs for one request.
    pub fn inputs(&self) -> Inputs {
        let (evaluated, deriving) = (Arc::clone(&self.evaluated), Arc::clone(&self.deriving));
        Inputs::shared(Arc::clone(&self.inputs), evaluated, deriving)
    }

    /// Adds the work of a request, or of the background, to what the next response reports.
    pub fn add(&self, work: &WorkCounts) {
        *lock(&self.work) += work;
    }

    /// The work done since the previous response, which a response reports.
    pub fn take_work(&self) -> WorkCounts {
        std::mem::take(&mut *lock(&self.work))
    }

    /// The directories that may hold inputs, as the last check listed them, with `.mara`,
    /// which holds the configuration, and the schema's directory: what the watcher watches.
    pub fn directories(&self) -> Vec<PathBuf> {
        let inputs = lock(&self.inputs);
        let mut directories: Vec<PathBuf> = (inputs.directories().into_iter())
            .map(|relative| self.root.join(relative))
            .collect();
        directories.push(self.root.join(".mara"));
        let schema = inputs.schema_path();
        drop(inputs);
        if let Some(parent) = schema.as_deref().map(Path::new).and_then(Path::parent) {
            directories.push(self.root.join(parent));
        }
        directories.sort_unstable();
        directories.dedup();
        directories
    }

    /// Checks the inputs and re-derives what changed, as a request that reads documents would,
    /// so that the next request finds the work done ([`Inputs::rederive`]). Its work goes to the
    /// next response: it is added before the derived state is given to other requests, so the
    /// response of a request that uses that state reports it.
    pub fn derive(&self) {
        let now = SystemTime::now();
        self.inputs().rederive(now, |work| {
            self.add(work);
            derived(work, now);
        });
    }

    /// [`Session::derive`] on a thread of its own, then `after`. A change that comes while one
    /// runs makes it run once more; at most one runs per session. It waits while a write of the
    /// session is between its first rename and its hand-over ([`Deriving::window`]).
    pub fn derive_in_background(self: &Arc<Self>, after: impl Fn(&Session) + Send + 'static) {
        {
            let mut state = lock(&self.deriving.state);
            if state.started {
                state.again = true;
                return;
            }
            state.started = true;
        }
        let session = Arc::clone(self);
        let spawned = thread::Builder::new()
            .name("mara-derive".to_owned())
            .spawn(move || {
                loop {
                    session.deriving.begin();
                    session.derive();
                    after(&session);
                    if !session.deriving.end() {
                        return;
                    }
                }
            });
        if spawned.is_err() {
            // The next request checks the inputs all the same.
            lock(&self.deriving.state).started = false;
        }
    }

    /// Marks a request as begun or ended: the idle period runs from the last of them.
    fn touch(&self, begins: bool) {
        let mut activity = lock(&self.activity);
        activity.last = Instant::now();
        match begins {
            true => activity.active += 1,
            false => activity.active -= 1,
        }
    }

    /// When the session falls idle, if no request is being served.
    fn idle_from(&self) -> Option<Instant> {
        let activity = lock(&self.activity);
        (activity.active == 0).then_some(activity.last)
    }
}

/// Appends the counts of a background derivation, once it gave its state to the session, to
/// the file that `MARA_TEST_DERIVED` names (ADR-DEBUG-PAUSE-POINT), so that a test can wait for
/// the watcher's derivation instead of a deadline: one line of the file reads, hashes, parses and
/// snapshot builds, then the time its check took as now, in nanoseconds since the epoch, which
/// tells whether it settled what it read.
fn derived(work: &WorkCounts, now: SystemTime) {
    let checked = now
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    hooks::append(
        Hook::Derived,
        format_args!(
            "{} {} {} {} {}",
            work.file_reads,
            work.hashes,
            work.parses,
            work.snapshot_builds,
            checked.as_nanos()
        ),
    );
}

/// A session's background derivation and the writes it serves, kept apart: a write's window
/// runs from its first rename to its hand-over ([`Inputs::apply`]), and no derivation runs
/// meanwhile, so that none reads and parses a file the write is about to hand over with its
/// read-back parse. Reads never wait here.
#[derive(Debug, Default)]
pub struct Deriving {
    state: Mutex<DerivingState>,
    changed: Condvar,
}

#[derive(Debug, Default)]
struct DerivingState {
    /// A derivation's thread exists.
    started: bool,
    /// A change came while it ran or waited: it runs once more.
    again: bool,
    /// It is deriving now.
    running: bool,
    /// The writes in their window.
    writes: usize,
}

impl Deriving {
    /// Opens a write's window: waits for a derivation in progress to end, and keeps the next
    /// from starting until the window is dropped.
    pub fn window(self: &Arc<Self>) -> Window {
        let mut state = lock(&self.state);
        while state.running {
            state = (self.changed.wait(state)).unwrap_or_else(PoisonError::into_inner);
        }
        state.writes += 1;
        Window(Arc::clone(self))
    }

    /// Waits until no write is in its window, then marks a derivation as running.
    fn begin(&self) {
        let mut state = lock(&self.state);
        while state.writes > 0 {
            state = (self.changed.wait(state)).unwrap_or_else(PoisonError::into_inner);
        }
        state.running = true;
    }

    /// Marks the derivation as ended: whether it is to run once more.
    fn end(&self) -> bool {
        let mut state = lock(&self.state);
        state.running = false;
        self.changed.notify_all();
        let again = std::mem::take(&mut state.again);
        state.started = again;
        again
    }
}

/// A write's window ([`Deriving::window`]), closed when dropped.
#[derive(Debug)]
pub struct Window(Arc<Deriving>);

impl Drop for Window {
    fn drop(&mut self) {
        lock(&self.0.state).writes -= 1;
        self.0.changed.notify_all();
    }
}

/// How many bytes of outcomes a session keeps at most ([`Evaluated`]). The number is
/// provisional.
const EVALUATED_BYTES: usize = 64 << 20;

/// What an estimate counts for each outcome kept, besides its rows: its key, its place in the
/// store and its allocation, so that outcomes without rows count too.
const ENTRY_BYTES: usize = 256;

/// The outcomes of a session's evaluations by key (DES-RULE-EXECUTION, DES-DAEMON-SESSIONS): a
/// request whose evaluation an earlier request of the session ran with the same key reads that
/// run. Only outcomes that the same key always gives are kept: rows and compile errors, never a
/// stop at a limit, a runtime fault or a fault of the host, which may depend on the machine at
/// the time. At most [`EVALUATED_BYTES`] of them, by an estimate, the oldest dropped first. An
/// outcome is kept for the Soufflé executable that gave it ([`Build`]), so a replaced one
/// evaluates again.
///
/// The outcomes of runs in provenance mode ([`rules::explain`]) are kept under the same rules,
/// apart from evaluations, by key and by the rows explained: a report reads one whose rows
/// explained are those its own selection chooses from the kept rows. Only a settled one is kept
/// ([`Explained::settled`]), whose every explanation the key and those rows decide.
#[derive(Debug, Default)]
pub struct Evaluated(Mutex<Kept>);

/// The outcome of an evaluation.
type Evaluation = Arc<Result<Rows, Fault>>;

/// The outcome of a run in provenance mode.
type Explanations = Arc<Result<Explained, Fault>>;

#[derive(Debug, Default)]
struct Kept {
    outcomes: HashMap<Key, Evaluation>,
    /// The outcomes of runs in provenance mode by key: those of one key have the same rows and
    /// differ in the rows explained.
    explained: HashMap<Key, Vec<Explanations>>,
    /// Every outcome kept, oldest first, with its size.
    order: VecDeque<(Key, Outcome, usize)>,
    bytes: usize,
}

/// One outcome kept.
#[derive(Clone, Debug)]
enum Outcome {
    Evaluation(Evaluation),
    Explanations(Explanations),
}

/// An outcome's key in the store: the evaluation's key ([`mara_engine::evaluation::key`]) and
/// the Soufflé that ran it.
type Key = (Hash, Build);

/// The identity of a Soufflé executable, as a daemon's build is identified
/// (DES-DAEMON-LIFECYCLE): its path, size and modification time.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Build {
    path: PathBuf,
    size: u64,
    modified: Option<SystemTime>,
}

impl Build {
    /// The identity of the executable at `path`, `None` when it cannot be read.
    fn of(path: &Path) -> Option<Build> {
        let metadata = fs::metadata(path).ok()?;
        Some(Build {
            path: path.to_owned(),
            size: metadata.len(),
            modified: metadata.modified().ok(),
        })
    }
}

impl Evaluated {
    /// The outcome of `run`, whose key is `key`: one that an earlier run with that key and the
    /// same Soufflé executable left, else evaluated ([`rules::evaluate`]) without holding the
    /// store, so that no request waits for another's evaluation.
    pub fn evaluate(&self, key: Hash, run: &Run, work: &mut WorkCounts) -> Evaluation {
        // An executable that cannot be read leaves the evaluation to report it.
        let Some(build) = Build::of(run.souffle) else {
            return Arc::new(rules::evaluate(run, work));
        };
        let key = (key, build);
        if let Some(outcome) = lock(&self.0).outcomes.get(&key).cloned() {
            return outcome;
        }
        let outcome = Arc::new(rules::evaluate(run, work));
        self.keep_within(
            key,
            Outcome::Evaluation(Arc::clone(&outcome)),
            EVALUATED_BYTES,
        );
        outcome
    }

    /// The outcome of `run` in provenance mode, whose key is `key`, explaining the rows that
    /// `select` chooses: one that an earlier run with that key and the same Soufflé executable
    /// left, having explained the rows that `select` chooses from its rows, else run
    /// ([`rules::explain`]) without holding the store.
    pub fn explain(
        &self,
        key: Hash,
        run: &Run,
        select: &mut dyn FnMut(&Rows) -> Vec<Target>,
        work: &mut WorkCounts,
    ) -> Explanations {
        let Some(build) = Build::of(run.souffle) else {
            return Arc::new(rules::explain(run, select, work));
        };
        let key = (key, build);
        if let Some(outcome) = self.find_explained(&key, select) {
            return outcome;
        }
        let outcome = Arc::new(rules::explain(run, select, work));
        self.keep_within(
            key,
            Outcome::Explanations(Arc::clone(&outcome)),
            EVALUATED_BYTES,
        );
        outcome
    }

    /// The kept outcome of a run in provenance mode with `key` that explained the rows `select`
    /// chooses from its rows, or its compile error, which no choice of rows changes. `select`
    /// runs without holding the store.
    fn find_explained(
        &self,
        key: &Key,
        select: &mut dyn FnMut(&Rows) -> Vec<Target>,
    ) -> Option<Explanations> {
        let kept = lock(&self.0).explained.get(key)?.clone();
        let rows = match kept.first().map(|outcome| &**outcome) {
            Some(Ok(explained)) => &explained.rows,
            _ => return kept.into_iter().next(),
        };
        let targets = select(rows);
        (kept.iter())
            .find(|outcome| matches!(&***outcome, Ok(explained) if explained.targets == targets))
            .cloned()
    }

    /// Keeps `outcome` under `key` if it is kept at all and no such outcome is, dropping the
    /// oldest outcomes until those kept hold at most `budget` bytes.
    fn keep_within(&self, key: Key, outcome: Outcome, budget: usize) {
        let Some(size) = size(&outcome) else {
            return;
        };
        if size > budget {
            return;
        }
        let mut kept = lock(&self.0);
        let present = match &outcome {
            Outcome::Evaluation(_) => kept.outcomes.contains_key(&key),
            Outcome::Explanations(new) => {
                (kept.explained.get(&key).into_iter().flatten()).any(|old| match (&**old, &**new) {
                    (Ok(old), Ok(new)) => old.targets == new.targets,
                    (old, new) => old.is_err() && new.is_err(),
                })
            }
        };
        if present {
            return;
        }
        while kept.bytes + size > budget {
            let Some((oldest, dropped, freed)) = kept.order.pop_front() else {
                break;
            };
            kept.bytes -= freed;
            match dropped {
                Outcome::Evaluation(_) => {
                    kept.outcomes.remove(&oldest);
                }
                Outcome::Explanations(dropped) => {
                    let Some(outcomes) = kept.explained.get_mut(&oldest) else {
                        continue;
                    };
                    outcomes.retain(|outcome| !Arc::ptr_eq(outcome, &dropped));
                    if outcomes.is_empty() {
                        kept.explained.remove(&oldest);
                    }
                }
            }
        }
        match &outcome {
            Outcome::Evaluation(rows) => {
                kept.outcomes.insert(key.clone(), Arc::clone(rows));
            }
            Outcome::Explanations(explained) => {
                let outcomes = kept.explained.entry(key.clone()).or_default();
                outcomes.push(Arc::clone(explained));
            }
        }
        kept.order.push_back((key, outcome, size));
        kept.bytes += size;
    }
}

/// An estimate of the memory that `outcome` holds as kept, at least [`ENTRY_BYTES`], or `None`
/// for an outcome that is not kept: a fault other than a compile error, or a run in provenance
/// mode that is not settled.
fn size(outcome: &Outcome) -> Option<usize> {
    let held = match outcome {
        Outcome::Evaluation(outcome) => match &**outcome {
            Ok(rows) => rows_bytes(rows),
            Err(fault) => fault_bytes(fault)?,
        },
        Outcome::Explanations(outcome) => match &**outcome {
            Ok(explained) if explained.settled => {
                let targets = (explained.targets.iter())
                    .map(|target| target.relation.len() + values_bytes(&target.values));
                let explanations = (explained.explanations.iter()).map(|explanation| {
                    size_of::<Explanation>()
                        + match explanation {
                            Ok(node) => node_bytes(node),
                            Err(reason) => reason.len(),
                        }
                });
                rows_bytes(&explained.rows)
                    + targets.sum::<usize>()
                    + explained.targets.len() * size_of::<Target>()
                    + explanations.sum::<usize>()
            }
            Ok(_) => return None,
            Err(fault) => fault_bytes(fault)?,
        },
    };
    Some(ENTRY_BYTES + held)
}

/// What the rows of each relation of `rows` hold.
fn rows_bytes(rows: &Rows) -> usize {
    (rows.relations.iter())
        .map(|(name, rows)| {
            let values: usize = rows.iter().map(|row| values_bytes(row)).sum();
            name.len() + rows.len() * size_of::<Vec<Value>>() + values
        })
        .sum()
}

/// What `values` hold, besides the vector that holds them.
fn values_bytes(values: &[Value]) -> usize {
    (values.iter())
        .map(|value| match value {
            Value::Symbol(text) => size_of::<Value>() + text.len(),
            Value::Number(_) => size_of::<Value>(),
        })
        .sum()
}

/// What a compile error holds, or `None` for any other fault, which is not kept.
fn fault_bytes(fault: &Fault) -> Option<usize> {
    match fault {
        Fault::Compile(messages) => Some(
            (messages.iter())
                .map(|message| size_of_val(message) + message.text.len())
                .sum(),
        ),
        _ => None,
    }
}

/// What a step of a proof holds, its children included.
fn node_bytes(node: &Node) -> usize {
    size_of::<Node>()
        + match node {
            Node::Derived {
                relation,
                atom,
                rule,
                children,
            } => {
                relation.len()
                    + atom.len()
                    + rule.len()
                    + children.iter().map(node_bytes).sum::<usize>()
            }
            Node::Fact {
                relation,
                values,
                path,
                ..
            } => relation.len() + values_bytes(values) + path.len(),
            Node::Input { name, values } => name.len() + values_bytes(values),
            Node::Absent { atom } => atom.len(),
        }
}

/// The open sessions of a daemon.
#[derive(Debug, Default)]
pub struct Sessions {
    open: Mutex<HashMap<PathBuf, Arc<Session>>>,
    /// Whether any session was opened: then the daemon exits with its last session.
    opened: Mutex<bool>,
}

/// A request being served in its session: it keeps the session from being dropped.
#[derive(Debug)]
pub struct Entered(Arc<Session>);

impl std::ops::Deref for Entered {
    type Target = Arc<Session>;

    fn deref(&self) -> &Arc<Session> {
        &self.0
    }
}

impl Drop for Entered {
    fn drop(&mut self) {
        self.0.touch(false);
    }
}

impl Sessions {
    /// The session of the canonical root `root`, opened unless it is open, and whether it was
    /// just opened.
    pub fn open(&self, root: &Path) -> (Arc<Session>, bool) {
        let mut open = lock(&self.open);
        self.find_or_open(&mut open, root)
    }

    /// Enters the session of `root` for a request, opening it unless it is open, and whether
    /// it was just opened. The session is not dropped while the request is served.
    pub fn enter(&self, root: &Path) -> (Entered, bool) {
        let mut open = lock(&self.open);
        let (session, opened) = self.find_or_open(&mut open, root);
        // Under the lock that dropping an idle session takes.
        session.touch(true);
        (Entered(session), opened)
    }

    fn find_or_open(
        &self,
        open: &mut HashMap<PathBuf, Arc<Session>>,
        root: &Path,
    ) -> (Arc<Session>, bool) {
        if let Some(session) = open.get(root) {
            return (Arc::clone(session), false);
        }
        let session = Arc::new(Session::new(root.to_owned()));
        open.insert(root.to_owned(), Arc::clone(&session));
        *lock(&self.opened) = true;
        (session, true)
    }

    /// The roots of the open sessions, in order.
    pub fn roots(&self) -> Vec<PathBuf> {
        let mut roots: Vec<PathBuf> = lock(&self.open).keys().cloned().collect();
        roots.sort_unstable();
        roots
    }

    /// Whether any session was ever opened.
    pub fn opened(&self) -> bool {
        *lock(&self.opened)
    }

    /// Drops each session idle for `idle` at `now`, with its inputs. Returns the dropped
    /// sessions and when the next one falls due, if any session stays open.
    pub fn drop_idle(&self, idle: Duration, now: Instant) -> (Vec<Arc<Session>>, Option<Instant>) {
        let mut open = lock(&self.open);
        let mut dropped = Vec::new();
        let mut next: Option<Instant> = None;
        open.retain(|_, session| {
            let due = session.idle_from().map(|last| last + idle);
            match due {
                Some(due) if due <= now => {
                    dropped.push(Arc::clone(session));
                    false
                }
                // A session serving a request falls due an idle period after it ends.
                due => {
                    let due = due.unwrap_or(now + idle);
                    next = Some(next.map_or(due, |next| next.min(due)));
                    true
                }
            }
        });
        (dropped, next)
    }

    /// Whether no session is open.
    pub fn is_empty(&self) -> bool {
        lock(&self.open).is_empty()
    }
}

#[cfg(test)]
mod tests {
    use mara_engine::evaluation::{Limit, Limits};

    use super::*;

    fn rows(text: &str) -> Outcome {
        Outcome::Evaluation(Arc::new(Ok(some_rows(text))))
    }

    fn some_rows(text: &str) -> Rows {
        let mut rows = Rows::default();
        let values = vec![vec![Value::Symbol(text.to_owned())]];
        rows.relations.insert("r".to_owned(), values);
        rows
    }

    fn target(text: &str) -> Target {
        Target {
            relation: "r".to_owned(),
            values: vec![Value::Symbol(text.to_owned())],
        }
    }

    /// A run in provenance mode whose rows hold `a` and `b` and which explained `targets`.
    fn explained(targets: &[&str], settled: bool) -> Explanations {
        let mut rows = some_rows("a");
        rows.relations
            .get_mut("r")
            .unwrap()
            .push(vec![Value::Symbol("b".to_owned())]);
        let explanations = (targets.iter())
            .map(|text| {
                Ok(Node::Absent {
                    atom: format!("r(\"{text}\")"),
                })
            })
            .collect();
        Arc::new(Ok(Explained {
            rows,
            targets: targets.iter().map(|text| target(text)).collect(),
            explanations,
            settled,
        }))
    }

    fn build() -> Build {
        Build {
            path: PathBuf::from("/souffle"),
            size: 1,
            modified: None,
        }
    }

    fn key(n: u8) -> Key {
        (Hash::of_bytes(&[n]), build())
    }

    fn kept(evaluated: &Evaluated) -> Vec<Key> {
        let kept = lock(&evaluated.0);
        kept.order.iter().map(|(key, ..)| key.clone()).collect()
    }

    /// Rows and compile errors are kept, the oldest dropped first past the budget; a fault that
    /// the machine may have caused is never kept, nor an outcome larger than the budget.
    #[test]
    fn only_outcomes_that_the_key_decides_are_kept_within_the_budget() {
        let evaluated = Evaluated::default();
        let one = rows("a");
        let size = super::size(&one).unwrap();
        let budget = 2 * size;
        evaluated.keep_within(key(1), one, budget);
        evaluated.keep_within(key(2), rows("b"), budget);
        assert_eq!(kept(&evaluated), [key(1), key(2)]);
        evaluated.keep_within(key(3), rows("c"), budget);
        assert_eq!(kept(&evaluated), [key(2), key(3)], "the oldest goes first");
        assert_eq!(lock(&evaluated.0).bytes, budget);

        let faults = [
            Fault::Host("cannot run Soufflé".to_owned()),
            Fault::Limit(Limit::Time, Limits::with_time(1)),
            Fault::Runtime {
                message: "Soufflé failed without a message".to_owned(),
                at: None,
            },
            Fault::Output {
                relation: "r".to_owned(),
                problem: "Soufflé wrote no readable file".to_owned(),
            },
        ];
        for (n, fault) in (4..).zip(faults) {
            evaluated.keep_within(key(n), Outcome::Evaluation(Arc::new(Err(fault))), budget);
        }
        evaluated.keep_within(key(9), rows(&"x".repeat(budget)), budget);
        assert_eq!(kept(&evaluated), [key(2), key(3)]);

        let compile = Outcome::Evaluation(Arc::new(Err(Fault::Compile(Vec::new()))));
        evaluated.keep_within(key(10), compile, budget);
        assert_eq!(kept(&evaluated).last(), Some(&key(10)));
    }

    /// An outcome without rows, as of a program run against empty facts with no output, costs
    /// a fixed amount: the budget bounds how many are kept.
    #[test]
    fn outcomes_without_rows_count_against_the_budget() {
        let evaluated = Evaluated::default();
        let empty = Outcome::Evaluation(Arc::new(Ok(Rows::default())));
        assert_eq!(super::size(&empty), Some(ENTRY_BYTES));
        for n in 0..10 {
            evaluated.keep_within(key(n), empty.clone(), 3 * ENTRY_BYTES);
        }
        assert_eq!(kept(&evaluated), [key(7), key(8), key(9)]);
    }

    /// An outcome kept for one Soufflé executable is not found once the executable at that
    /// path was replaced.
    #[test]
    fn an_outcome_is_kept_for_the_souffle_that_gave_it() {
        let dir = tempfile::tempdir().unwrap();
        let souffle = dir.path().join("souffle");
        fs::write(&souffle, "one build").unwrap();
        let before = Build::of(&souffle).unwrap();
        let evaluated = Evaluated::default();
        let hash = Hash::of_bytes(b"evaluation");
        evaluated.keep_within((hash, before.clone()), rows("a"), EVALUATED_BYTES);
        fs::write(&souffle, "another build").unwrap();
        let after = Build::of(&souffle).unwrap();
        assert_ne!(before, after);
        let find = |key| lock(&evaluated.0).outcomes.get(&key).cloned();
        assert!(find((hash, after)).is_none(), "the old outcome was found");
        assert!(find((hash, before)).is_some());
        assert!(Build::of(&dir.path().join("missing")).is_none());
    }

    /// A run in provenance mode is found by its key and by the rows that the request's
    /// selection chooses from its rows; a compile error whatever the selection; never an
    /// evaluation's rows, and never a run that left a row without the explanation its exchange
    /// would give.
    #[test]
    fn an_explained_outcome_is_found_for_the_rows_that_the_request_explains() {
        let evaluated = Evaluated::default();
        let budget = EVALUATED_BYTES;
        let choose = |chosen: &'static [&'static str]| {
            move |rows: &Rows| {
                assert_eq!(rows.relations["r"].len(), 2, "the kept run's rows");
                chosen.iter().map(|text| target(text)).collect::<Vec<_>>()
            }
        };
        let a = explained(&["a"], true);
        evaluated.keep_within(key(1), Outcome::Explanations(Arc::clone(&a)), budget);
        let found = evaluated.find_explained(&key(1), &mut choose(&["a"]));
        assert!(Arc::ptr_eq(&found.unwrap(), &a));
        assert!(
            evaluated
                .find_explained(&key(1), &mut choose(&["b"]))
                .is_none()
        );
        assert!(
            evaluated
                .find_explained(&key(1), &mut choose(&[]))
                .is_none()
        );
        assert!(
            evaluated
                .find_explained(&key(2), &mut choose(&["a"]))
                .is_none()
        );

        let b = explained(&["b"], true);
        evaluated.keep_within(key(1), Outcome::Explanations(Arc::clone(&b)), budget);
        let found = evaluated.find_explained(&key(1), &mut choose(&["b"]));
        assert!(Arc::ptr_eq(&found.unwrap(), &b));
        evaluated.keep_within(
            key(1),
            Outcome::Explanations(explained(&["b"], true)),
            budget,
        );
        assert_eq!(kept(&evaluated).len(), 2, "the same outcome is kept once");

        evaluated.keep_within(
            key(3),
            Outcome::Explanations(explained(&["a"], false)),
            budget,
        );
        assert!(
            evaluated
                .find_explained(&key(3), &mut choose(&["a"]))
                .is_none()
        );
        evaluated.keep_within(key(4), rows("a"), budget);
        assert!(
            evaluated
                .find_explained(&key(4), &mut choose(&["a"]))
                .is_none()
        );
        let limit = Fault::Limit(Limit::Time, Limits::with_time(1));
        evaluated.keep_within(key(5), Outcome::Explanations(Arc::new(Err(limit))), budget);
        assert!(
            evaluated
                .find_explained(&key(5), &mut choose(&[]))
                .is_none()
        );

        let compile: Explanations = Arc::new(Err(Fault::Compile(Vec::new())));
        evaluated.keep_within(key(6), Outcome::Explanations(Arc::clone(&compile)), budget);
        let found = evaluated.find_explained(&key(6), &mut |_: &Rows| panic!("no rows"));
        assert!(Arc::ptr_eq(&found.unwrap(), &compile));
    }

    /// Runs in provenance mode and evaluations share one budget and one order, and a dropped
    /// run is found no more.
    #[test]
    fn explained_outcomes_share_the_budget_with_evaluations() {
        let evaluated = Evaluated::default();
        let explanations = Outcome::Explanations(explained(&["a"], true));
        let budget = super::size(&explanations).unwrap() + super::size(&rows("a")).unwrap();
        evaluated.keep_within(key(1), explanations, budget);
        evaluated.keep_within(key(2), rows("a"), budget);
        assert_eq!(kept(&evaluated), [key(1), key(2)]);
        evaluated.keep_within(key(3), rows("b"), budget);
        assert_eq!(kept(&evaluated), [key(2), key(3)]);
        assert!(lock(&evaluated.0).explained.is_empty());
        let mut choose = |_: &Rows| vec![target("a")];
        assert!(evaluated.find_explained(&key(1), &mut choose).is_none());
    }
}
