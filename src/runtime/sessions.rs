//! Disposable native state. Every operation checks bytes/inventory, never mtimes.
use sha2::{Digest, Sha256};
use std::{
    any::Any,
    cell::RefCell,
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

/// Limits apply to idle projections, not canonical source or active operations.
#[derive(Debug, Clone)]
pub struct SessionPolicy {
    pub max_idle_sessions: usize,
    pub idle_timeout: Duration,
}
impl Default for SessionPolicy {
    fn default() -> Self {
        Self {
            max_idle_sessions: 16,
            idle_timeout: Duration::from_secs(300),
        }
    }
}
/// Cache-eligible read-loader work, not a cache-hit estimate or acceptance verdict.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionActivity {
    pub document_parses: usize,
    pub code_loads: usize,
    pub rule_compilations: usize,
    pub discovery_builds: usize,
    pub relation_builds: usize,
}
#[derive(Default)]
struct Derived {
    values: BTreeMap<String, Arc<dyn Any + Send + Sync>>,
    rule_lru: VecDeque<String>,
    recorded: BTreeSet<PathBuf>,
    initial_records: BTreeSet<PathBuf>,
    unverified_index_source: bool,
    fingerprint: Option<Vec<u8>>,
    activity: SessionActivity,
    observed: BTreeMap<PathBuf, Option<Vec<u8>>>,
    root: PathBuf,
}
impl Derived {
    fn clear_values(&mut self) {
        for value in self.values.values() {
            if let Some(corpus) = value.downcast_ref::<crate::Corpus>() {
                let activity = corpus.projection_activity();
                self.activity.discovery_builds += activity.discovery_builds;
                self.activity.relation_builds += activity.relation_builds;
            }
        }
        self.values.clear();
        self.rule_lru.clear();
    }
    fn clear(&mut self) {
        self.clear_values();
        self.initial_records.clear();
        self.unverified_index_source = false;
        self.recorded.clear();
        self.observed.clear();
    }
    fn retain_rule(&mut self, key: &str) {
        if key.starts_with("rules:") {
            self.rule_lru.retain(|entry| entry != key);
            self.rule_lru.push_back(key.to_owned());
            // Parameterized checks must not grow a continuously used root forever.
            if self.rule_lru.len() > 32 {
                let oldest = self.rule_lru.pop_front().unwrap();
                self.values.remove(&oldest);
            }
        }
    }
}
struct Session {
    gate: Mutex<()>,
    derived: Arc<Mutex<Derived>>,
    idle_since: Mutex<Instant>,
}
impl Session {
    fn new() -> Self {
        Self {
            gate: Mutex::new(()),
            derived: Arc::new(Mutex::new(Derived::default())),
            idle_since: Mutex::new(Instant::now()),
        }
    }
}
struct Pool {
    policy: SessionPolicy,
    sessions: BTreeMap<PathBuf, Arc<Session>>,
}
/// Share this pool between embedded runtimes or all project requests in a daemon.
// @mara implements DES-NATIVE-PROJECT-SESSIONS
// @mara implements REQ-FRESH-PROJECT-SESSIONS
#[derive(Clone)]
pub struct NativeSessions(Arc<Mutex<Pool>>);
impl std::fmt::Debug for NativeSessions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeSessions").finish_non_exhaustive()
    }
}
impl Default for NativeSessions {
    fn default() -> Self {
        Self::new(SessionPolicy::default())
    }
}
impl NativeSessions {
    pub fn new(policy: SessionPolicy) -> Self {
        Self(Arc::new(Mutex::new(Pool {
            policy,
            sessions: BTreeMap::new(),
        })))
    }
    /// Evict expired/LRU idle entries. Active calls hold a lease and cannot be removed.
    pub fn prune_idle(&self) -> usize {
        let mut pool = self.0.lock().unwrap();
        let before = pool.sessions.len();
        let timeout = pool.policy.idle_timeout;
        pool.sessions.retain(|_, s| {
            Arc::strong_count(s) != 1 || s.idle_since.lock().unwrap().elapsed() < timeout
        });
        let mut idle = pool
            .sessions
            .iter()
            .filter(|(_, s)| Arc::strong_count(s) == 1)
            .map(|(p, s)| (p.clone(), *s.idle_since.lock().unwrap()))
            .collect::<Vec<_>>();
        idle.sort_by_key(|(_, t)| *t);
        let excess = idle.len().saturating_sub(pool.policy.max_idle_sessions);
        for (p, _) in idle.into_iter().take(excess) {
            pool.sessions.remove(&p);
        }
        before - pool.sessions.len()
    }
    pub fn retained_sessions(&self) -> usize {
        self.0.lock().unwrap().sessions.len()
    }
    fn lease(&self, root: &Path) -> Arc<Session> {
        self.prune_idle();
        self.0
            .lock()
            .unwrap()
            .sessions
            .entry(root.to_owned())
            .or_insert_with(|| Arc::new(Session::new()))
            .clone()
    }
    pub(crate) fn refresh(&self, root: &Path) {
        let session = self.lease(root);
        let gate = session.gate.lock().unwrap();
        let mut d = session.derived.lock().unwrap();
        d.clear();
        d.fingerprint = None;
        drop(d);
        drop(gate);
        drop(session);
        self.prune_idle();
    }
    pub(crate) fn activity(&self, root: &Path) -> SessionActivity {
        self.0
            .lock()
            .unwrap()
            .sessions
            .get(root)
            .map(|s| {
                let d = s.derived.lock().unwrap();
                let mut activity = d.activity.clone();
                for value in d.values.values() {
                    if let Some(corpus) = value.downcast_ref::<crate::Corpus>() {
                        let p = corpus.projection_activity();
                        activity.discovery_builds += p.discovery_builds;
                        activity.relation_builds += p.relation_builds;
                    }
                }
                activity
            })
            .unwrap_or_default()
    }
    pub(crate) fn run<T, E>(
        &self,
        root: &Path,
        write: bool,
        f: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, E> {
        let session = self.lease(root);
        let gate = session.gate.lock().unwrap();
        let (usable, original_records, original_fingerprint) = {
            let mut d = session.derived.lock().unwrap();
            d.observed.clear();
            d.root = root.to_owned();
            let mut current = fingerprint(root, &d.recorded).ok();
            if current.is_none() || current != d.fingerprint || write {
                d.clear();
                // Rebuild dependencies from current loaders, not retired assets.
                current = fingerprint(root, &d.recorded).ok();
            }
            d.fingerprint = current;
            d.initial_records = d.recorded.clone();
            d.unverified_index_source = false;
            (
                d.fingerprint.is_some() && !write,
                d.recorded.clone(),
                d.fingerprint.clone(),
            )
        };
        let scope = Scope::enter(usable.then(|| session.derived.clone()));
        let result = f();
        drop(scope);
        if result.is_err() || write {
            let mut d = session.derived.lock().unwrap();
            d.clear();
            d.fingerprint = None;
        } else {
            // Loaders can record ignored/explicit assets during the operation.
            let mut d = session.derived.lock().unwrap();
            let stable = fingerprint(root, &original_records).ok() == original_fingerprint
                && d.observed
                    .iter()
                    .all(|(p, before)| input_identity(root, p) == *before);
            if !stable {
                d.clear();
            } else if d.unverified_index_source {
                // Keep learned inputs, not projections: the next operation can
                // verify them both before and after a fresh indexer execution.
                d.clear_values();
            }
            d.fingerprint = if stable {
                fingerprint(root, &d.recorded).ok()
            } else {
                None
            };
        }
        *session.idle_since.lock().unwrap() = Instant::now();
        drop(gate);
        drop(session);
        self.prune_idle();
        result
    }
}
thread_local! { static ACTIVE: RefCell<Option<Arc<Mutex<Derived>>>> = const { RefCell::new(None) }; }
struct Scope(Option<Arc<Mutex<Derived>>>);
impl Scope {
    fn enter(value: Option<Arc<Mutex<Derived>>>) -> Self {
        Self(ACTIVE.with(|a| a.replace(value)))
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        ACTIVE.with(|a| a.replace(self.0.take()));
    }
}
fn active() -> Option<Arc<Mutex<Derived>>> {
    ACTIVE.with(|a| a.borrow().clone())
}
pub(crate) fn unverified_index_source(path: &Path) {
    ACTIVE.with(|a| {
        if let Some(a) = a.borrow().as_ref() {
            let mut d = a.lock().unwrap();
            if !d.initial_records.contains(path) {
                d.unverified_index_source = true;
            }
        }
    });
}
pub(crate) fn record(path: PathBuf) {
    if let Some(a) = active() {
        let mut d = a.lock().unwrap();
        if !path
            .strip_prefix(&d.root)
            .is_ok_and(crate::is_project_relative)
        {
            return;
        }
        let identity = input_identity(&d.root, &path);
        d.observed.entry(path.clone()).or_insert(identity);
        d.recorded.insert(path);
    }
}
fn input_identity(root: &Path, path: &Path) -> Option<Vec<u8>> {
    let mut hash = Sha256::new();
    match fs::canonicalize(path) {
        Ok(p) => {
            frame(&mut hash, p.as_os_str().as_encoded_bytes());
            if !p.starts_with(root) || !p.is_file() {
                hash.update(b"not-a-contained-file");
            } else {
                hash.update([1]);
                frame(&mut hash, &fs::read(p).ok()?);
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => hash.update([0]),
        Err(_) => return None,
    }
    Some(hash.finalize().to_vec())
}
pub(crate) fn count(kind: &str, amount: usize) {
    if let Some(a) = active() {
        let mut d = a.lock().unwrap();
        match kind {
            "documents" => d.activity.document_parses += amount,
            "code" => d.activity.code_loads += amount,
            "rules" => d.activity.rule_compilations += amount,
            _ => {}
        }
    }
}
pub(crate) fn cached<T: Clone + Send + Sync + 'static, E>(
    key: String,
    load: impl FnOnce() -> Result<T, E>,
) -> Result<T, E> {
    let cache = active();
    if let Some(a) = &cache {
        let mut d = a.lock().unwrap();
        if let Some(value) = d
            .values
            .get(&key)
            .and_then(|v| v.downcast_ref::<T>())
            .cloned()
        {
            d.retain_rule(&key);
            return Ok(value);
        }
    }
    let value = load()?;
    if let Some(a) = cache {
        let mut d = a.lock().unwrap();
        d.values.insert(key.clone(), Arc::new(value.clone()));
        d.retain_rule(&key);
    }
    Ok(value)
}
pub(crate) fn key(label: &str, bytes: &[u8]) -> String {
    format!("{label}:{:x}", Sha256::digest(bytes))
}
fn fingerprint(root: &Path, recorded: &BTreeSet<PathBuf>) -> Result<Vec<u8>, String> {
    let inputs = crate::code::input_snapshot(root)?;
    let mut hash = Sha256::new();
    for (path, bytes) in inputs {
        frame(&mut hash, path.as_os_str().as_encoded_bytes());
        frame(&mut hash, &bytes);
    }
    for path in recorded {
        frame(&mut hash, path.as_os_str().as_encoded_bytes());
        frame(
            &mut hash,
            &input_identity(root, path).ok_or("could not inspect recorded input")?,
        );
    }
    for path in root.ancestors().map(|p| p.join(".gitignore")) {
        frame(&mut hash, path.as_os_str().as_encoded_bytes());
        match fs::read(&path) {
            Ok(bytes) => {
                hash.update([1]);
                frame(&mut hash, &bytes);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => hash.update([0]),
            Err(e) => return Err(e.to_string()),
        }
        // Retargeted symlinks and file/containment changes must not reuse success.
        match fs::canonicalize(&path) {
            Ok(p) => frame(&mut hash, p.as_os_str().as_encoded_bytes()),
            Err(_) => hash.update([0]),
        }
    }
    Ok(hash.finalize().to_vec())
}
fn frame(hash: &mut Sha256, bytes: &[u8]) {
    hash.update(bytes.len().to_le_bytes());
    hash.update(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    // @mara checks DES-NATIVE-PROJECT-SESSIONS
    #[test]
    fn ignored_config_changes_retire_schema_only_project_variants() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(fixture.path()).unwrap();
        crate::initialize_project(&root, crate::Template::Minimal).unwrap();
        fs::write(root.join(".gitignore"), ".mara/\n").unwrap();
        let config = root.join(crate::PROJECT_FILE);
        let original = fs::read_to_string(&config).unwrap();
        let pool = NativeSessions::default();
        let runtime = crate::NativeRuntime::new(Some(root.clone()), root.clone())
            .unwrap()
            .with_sessions(pool.clone());
        for revision in 0..8 {
            fs::write(&config, format!("{original}\n# revision {revision}\n")).unwrap();
            runtime.schema_get(None, None).unwrap();
            let sessions = pool.0.lock().unwrap();
            let derived = sessions.sessions[&root].derived.lock().unwrap();
            assert_eq!(
                derived
                    .values
                    .keys()
                    .filter(|key| key.starts_with("project:"))
                    .count(),
                1
            );
        }
        let sessions = pool.0.lock().unwrap();
        assert!(
            sessions.sessions[&root]
                .derived
                .lock()
                .unwrap()
                .recorded
                .contains(&config)
        );
    }
}
