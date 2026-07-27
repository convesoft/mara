use clap::{Args, Parser, Subcommand};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
#[cfg(test)]
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::env;
use std::ffi::CString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, Instant};

const ITEM_COUNT: usize = 10_000;
const DOCUMENT_COUNT: usize = 10;
const EDGES_PER_ITEM: usize = 10;
const RUN_COUNT: usize = 5;
const MINIMUM_AVAILABLE_BYTES: u64 = 2_147_483_648;
const ELAPSED_LIMIT_NS: u64 = 5_000_000_000;
const PEAK_RSS_LIMIT_KIB: u64 = 524_288;

const PROJECT_TOML: &str = "format_version = 1\n\n[project]\nname = \"mara-scale-v01\"\nschema = \".mara/schema.yaml\"\n\n[content]\ninclude = [\"items-*.mara.md\"]\nexclude = []\nrespect_gitignore = false\nfollow_directory_symlinks = false\nallow_internal_file_symlinks = false\n\n[index]\npath = \".mara/index.json\"\n\n[validation]\nwarnings_as_errors = true\n\n[git]\nrequire_clean_worktree_for_writes = true\n";

const SCHEMA_YAML: &str = "format_version: 1\nschema:\n  name: mara-scale-v01\n  version: 0.1.0\nidentity:\n  mid:\n    format: ulid\n    prefix: m_\nflavours:\n  scale:\n    label: Scale item\n    description: Deterministic v0.1 qualification item.\n    guidance:\n      use_when:\n        - Measuring the v0.1 scale qualification workload.\n      avoid_when:\n        - Representing non-qualification content.\n    id:\n      required: true\n      pattern: 'SCALE-[0-9]{5}'\n    title:\n      required: true\n    body:\n      required: false\nrelations:\n  depends_on:\n    source:\n      flavours: [scale]\n    target:\n      flavours: [scale]\n    same_flavour: true\n    self_reference: false\n    cardinality:\n      outgoing:\n        min: 10\n        max: 10\n      incoming:\n        min: 10\n        max: 10\nrules: []\n";

#[derive(Parser)]
#[command(
    name = "mara-xtask",
    version,
    about = "Bounded Mara v0.1 qualification tooling"
)]
struct Cli {
    #[command(subcommand)]
    command: TopLevelCommand,
}

#[derive(Subcommand)]
enum TopLevelCommand {
    Qualification(Qualification),
}

#[derive(Args)]
struct Qualification {
    #[command(subcommand)]
    command: QualificationCommand,
}

#[derive(Subcommand)]
enum QualificationCommand {
    GenerateScaleV01 {
        #[arg(long)]
        qualification_root: PathBuf,
    },
    MeasureScaleV01 {
        #[arg(long)]
        qualification_root: PathBuf,
    },
}

#[derive(Debug)]
struct ToolError(String);

impl std::fmt::Display for ToolError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl std::error::Error for ToolError {}

type Result<T> = std::result::Result<T, ToolError>;

#[derive(Debug, Clone)]
struct StorageInfo {
    filesystem_type: String,
    total_bytes: u64,
    available_bytes: u64,
    on_tmpfs_or_ramfs: bool,
}

fn storage_precondition_error(
    storage: &StorageInfo,
    inside_source_control: bool,
) -> Option<&'static str> {
    if inside_source_control {
        Some("inside_source_control")
    } else if storage.on_tmpfs_or_ramfs {
        Some("tmpfs_or_ramfs")
    } else if storage.available_bytes < MINIMUM_AVAILABLE_BYTES {
        Some("insufficient_capacity")
    } else {
        None
    }
}

#[derive(Serialize)]
struct PreconditionRecord<'a> {
    format: &'static str,
    version: u8,
    source_repo: Option<&'a str>,
    qualification_parent: Option<&'a str>,
    qualification_root: Option<&'a str>,
    filesystem_type: Option<&'a str>,
    total_bytes: Option<u64>,
    available_bytes: Option<u64>,
    minimum_available_bytes: u64,
    on_tmpfs_or_ramfs: Option<bool>,
    inside_source_control: Option<bool>,
    error: &'a str,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct StorageRecord {
    source_repo: String,
    qualification_parent: String,
    qualification_root: String,
    filesystem_type: String,
    total_bytes: u64,
    available_bytes: u64,
    minimum_available_bytes: u64,
    on_tmpfs_or_ramfs: bool,
    inside_source_control: bool,
    passed: bool,
}

#[derive(Debug, Clone, Serialize)]
struct FixtureFile {
    path: String,
    expected_sha256: String,
    observed_sha256: Option<String>,
    matched: bool,
}

#[derive(Debug, Clone, Serialize)]
struct MeasurementRecord {
    run: u8,
    elapsed_ns: Option<u64>,
    peak_rss_kib: Option<u64>,
    exit_code: Option<i32>,
    term_signal: Option<i32>,
    timed_out: bool,
    stdout_sha256: Option<String>,
    stderr_sha256: Option<String>,
    mara_status: Option<String>,
    diagnostic_count: Option<u64>,
    documents: Option<u64>,
    items: Option<u64>,
    edges: Option<u64>,
    fixture_verified: Option<bool>,
    passed: bool,
    error: Option<String>,
}

#[derive(Serialize)]
struct QualificationSummary {
    format: &'static str,
    version: u8,
    source_commit: String,
    xtask_sha256: String,
    mara_sha256: String,
    expected_manifest_sha256: Option<String>,
    fixture_files: Vec<FixtureFile>,
    runs: Vec<MeasurementRecord>,
    max_elapsed_ns: Option<u64>,
    max_peak_rss_kib: Option<u64>,
    elapsed_limit_ns: u64,
    peak_rss_limit_kib: u64,
    result: &'static str,
}

struct SummaryInputs {
    source_commit: String,
    xtask_sha256: String,
    mara_sha256: String,
    expected_manifest_sha256: Option<String>,
    fixture_files: Vec<FixtureFile>,
    records: Vec<MeasurementRecord>,
    result: &'static str,
}

#[derive(Debug, Clone)]
struct ManifestPin {
    sha256: String,
    entries: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
struct FixtureRevalidation {
    files: Vec<FixtureFile>,
    integrity_matches: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        TopLevelCommand::Qualification(qualification) => match qualification.command {
            QualificationCommand::GenerateScaleV01 { qualification_root } => {
                generate_scale_v01(&qualification_root)
            }
            QualificationCommand::MeasureScaleV01 { qualification_root } => {
                measure_scale_v01(&qualification_root)
            }
        },
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("mara-xtask: {error}");
            ExitCode::FAILURE
        }
    }
}

fn generate_scale_v01(argument_root: &Path) -> Result<()> {
    let source_repo = source_repository()?;
    canonical_xtask_executable(&source_repo)?;
    let candidate = candidate_root(argument_root, false)?;
    let inside_source_control = path_is_in_source_control(&candidate.root, &source_repo)?;
    if let Err(error) = ensure_isolated_root(&candidate.root, &source_repo, &candidate.parent) {
        let precondition_error = if inside_source_control {
            "inside_source_control"
        } else {
            "root_not_isolated"
        };
        emit_precondition(
            Some(&source_repo),
            Some(&candidate.parent),
            Some(&candidate.root),
            None,
            inside_source_control,
            precondition_error,
        )?;
        return Err(error);
    }
    let storage = match storage_info(&candidate.parent) {
        Ok(storage) => storage,
        Err(error) => {
            emit_precondition(
                Some(&source_repo),
                Some(&candidate.parent),
                Some(&candidate.root),
                None,
                inside_source_control,
                "storage_unavailable",
            )?;
            return Err(error);
        }
    };

    if let Some(error) = storage_precondition_error(&storage, inside_source_control) {
        emit_precondition(
            Some(&source_repo),
            Some(&candidate.parent),
            Some(&candidate.root),
            Some(&storage),
            inside_source_control,
            error,
        )?;
        return Err(ToolError(error.into()));
    }

    fs::create_dir(&candidate.root).map_err(|error| {
        ToolError(format!(
            "cannot create qualification root {}: {error}",
            candidate.root.display()
        ))
    })?;
    if let Err(error) = require_real_directory(&candidate.root, "qualification root") {
        return Err(rollback_created_root(&candidate.root, error.to_string()));
    }
    let root = match fs::canonicalize(&candidate.root) {
        Ok(root) => root,
        Err(error) => {
            return Err(rollback_created_root(
                &candidate.root,
                format!(
                    "cannot canonicalize qualification root {}: {error}",
                    candidate.root.display()
                ),
            ));
        }
    };
    if root != candidate.root {
        return Err(rollback_created_root(
            &candidate.root,
            "qualification root changed after creation".into(),
        ));
    }
    if let Err(error) = require_real_directory(&candidate.root, "qualification root") {
        return Err(rollback_created_root(&candidate.root, error.to_string()));
    }
    if let Err(error) = ensure_isolated_root(&root, &source_repo, &candidate.parent) {
        return Err(rollback_created_root(&candidate.root, error.to_string()));
    }
    let created_storage = match storage_info(&root) {
        Ok(storage) => storage,
        Err(error) => {
            return Err(reject_created_root(
                &source_repo,
                &candidate,
                &root,
                None,
                "storage_unavailable",
                error.to_string(),
            ));
        }
    };
    if let Some(error) = storage_precondition_error(&created_storage, false) {
        return Err(reject_created_root(
            &source_repo,
            &candidate,
            &root,
            Some(&created_storage),
            error,
            format!("qualification root no longer satisfies storage preconditions: {error}"),
        ));
    }

    let fixture = root.join("fixture");
    let evidence = root.join("evidence");
    fs::create_dir(&fixture)
        .map_err(|error| ToolError(format!("cannot create fixture directory: {error}")))?;
    fs::create_dir(&evidence)
        .map_err(|error| ToolError(format!("cannot create evidence directory: {error}")))?;

    let accepted_storage = StorageRecord {
        source_repo: path_text(&source_repo),
        qualification_parent: path_text(&candidate.parent),
        qualification_root: path_text(&root),
        filesystem_type: created_storage.filesystem_type.clone(),
        total_bytes: created_storage.total_bytes,
        available_bytes: created_storage.available_bytes,
        minimum_available_bytes: MINIMUM_AVAILABLE_BYTES,
        on_tmpfs_or_ramfs: created_storage.on_tmpfs_or_ramfs,
        inside_source_control: false,
        passed: true,
    };
    write_json(
        &evidence.join("storage-before-generation.json"),
        &accepted_storage,
    )?;
    write_fixture(&fixture)?;
    Ok(())
}

fn rollback_created_root(root: &Path, cause: String) -> ToolError {
    match fs::remove_dir(root) {
        Ok(()) => ToolError(cause),
        Err(error) => ToolError(format!(
            "{cause}; cannot remove newly created qualification root {}: {error}",
            root.display()
        )),
    }
}

fn reject_created_root(
    source_repo: &Path,
    candidate: &CandidateRoot,
    root: &Path,
    storage: Option<&StorageInfo>,
    precondition_error: &str,
    cause: String,
) -> ToolError {
    let evidence_error = emit_precondition(
        Some(source_repo),
        Some(&candidate.parent),
        Some(root),
        storage,
        false,
        precondition_error,
    )
    .err();
    let rollback_error = rollback_created_root(&candidate.root, cause);
    match evidence_error {
        Some(error) => ToolError(format!(
            "{rollback_error}; cannot emit precondition record: {error}"
        )),
        None => rollback_error,
    }
}

fn validate_generation_storage_record(
    evidence: &Path,
    source_repo: &Path,
    qualification_parent: &Path,
    qualification_root: &Path,
) -> Result<()> {
    let record_path = evidence.join("storage-before-generation.json");
    require_regular_file(&record_path, "evidence/storage-before-generation.json")?;
    let contents = fs::read(&record_path).map_err(|error| {
        ToolError(format!(
            "cannot read generation storage record {}: {error}",
            record_path.display()
        ))
    })?;
    let record = serde_json::from_slice::<StorageRecord>(&contents).map_err(|error| {
        ToolError(format!(
            "cannot parse generation storage record {}: {error}",
            record_path.display()
        ))
    })?;
    if record.source_repo != path_text(source_repo)
        || record.qualification_parent != path_text(qualification_parent)
        || record.qualification_root != path_text(qualification_root)
        || record.filesystem_type.is_empty()
        || record.minimum_available_bytes != MINIMUM_AVAILABLE_BYTES
        || record.available_bytes < MINIMUM_AVAILABLE_BYTES
        || record.on_tmpfs_or_ramfs
        || record.inside_source_control
        || !record.passed
    {
        return Err(ToolError(
            "generation storage record does not prove the required preconditions".into(),
        ));
    }
    Ok(())
}

fn measure_scale_v01(argument_root: &Path) -> Result<()> {
    #[cfg(not(target_os = "linux"))]
    {
        let _ = argument_root;
        return Err(ToolError(
            "measure-scale-v01 is supported only on Linux".into(),
        ));
    }

    #[cfg(target_os = "linux")]
    {
        measure_scale_v01_linux(argument_root)
    }
}

#[cfg(target_os = "linux")]
fn measure_scale_v01_linux(argument_root: &Path) -> Result<()> {
    let source_repo = source_repository()?;
    let xtask = canonical_xtask_executable(&source_repo)?;
    let candidate = candidate_root(argument_root, true)?;
    let root = candidate.root;
    ensure_isolated_root(&root, &source_repo, &candidate.parent)?;
    ensure_root_has_no_git_entry(&root)?;
    let fixture = root.join("fixture");
    let evidence = root.join("evidence");
    require_real_directory(&fixture, "fixture")?;
    require_real_directory(&evidence, "evidence")?;
    ensure_fixture_has_no_git_context(&fixture)?;
    validate_generation_storage_record(&evidence, &source_repo, &candidate.parent, &root)?;

    let storage = storage_info(&root)?;
    if storage_precondition_error(&storage, false).is_some() {
        return Err(ToolError(
            "qualification root no longer satisfies storage preconditions".into(),
        ));
    }
    let storage_record = StorageRecord {
        source_repo: path_text(&source_repo),
        qualification_parent: path_text(&candidate.parent),
        qualification_root: path_text(&root),
        filesystem_type: storage.filesystem_type.clone(),
        total_bytes: storage.total_bytes,
        available_bytes: storage.available_bytes,
        minimum_available_bytes: MINIMUM_AVAILABLE_BYTES,
        on_tmpfs_or_ramfs: storage.on_tmpfs_or_ramfs,
        inside_source_control: false,
        passed: true,
    };
    write_json(
        &evidence.join("storage-before-measurement.json"),
        &storage_record,
    )?;

    let mara = source_repo.join("target/release/mara");
    require_regular_file(&mara, "target/release/mara")?;
    let manifest = source_repo.join("tests/qualification/scale-v01.SHA256SUMS");
    require_regular_file(&manifest, "tests/qualification/scale-v01.SHA256SUMS")?;
    let script = source_repo.join("tests/qualification/verify-scale-v01.sh");
    require_regular_file(&script, "tests/qualification/verify-scale-v01.sh")?;

    let source_commit = git_output(&source_repo, ["rev-parse", "HEAD"])?;
    let xtask_sha256 = hash_file(&xtask)?;
    let mara_sha256 = hash_file(&mara)?;

    let oracle = Command::new(&script)
        .current_dir(&source_repo)
        .arg("--qualification-root")
        .arg(&root)
        .status();
    let pin = match oracle {
        Err(_) => Err("oracle_unavailable"),
        Ok(status) if status.success() => {
            parse_manifest_pin(&evidence.join("manifest-path-check.txt"))
                .map_err(|_| "fixture_revalidation_unavailable")
        }
        Ok(_) => Err("oracle_failed"),
    };

    let mut records = Vec::with_capacity(RUN_COUNT);
    let pin = match pin {
        Err(oracle_error) => {
            let result = if oracle_error == "oracle_failed" {
                "failed"
            } else {
                "inconclusive"
            };
            for run in 1..=RUN_COUNT {
                let record = unavailable_record(run as u8, oracle_error);
                write_json(
                    &evidence.join(format!("run-{run:02}.measurement.json")),
                    &record,
                )?;
                records.push(record);
            }
            write_summary(
                &evidence,
                SummaryInputs {
                    source_commit,
                    xtask_sha256,
                    mara_sha256,
                    expected_manifest_sha256: None,
                    fixture_files: Vec::new(),
                    records,
                    result,
                },
            )?;
            return Err(ToolError(oracle_error.into()));
        }
        Ok(pin) => pin,
    };
    let expected_manifest_sha256 = Some(pin.sha256.clone());
    let mut fixture_files = pin
        .entries
        .iter()
        .map(|(path, expected_sha256)| FixtureFile {
            path: path.clone(),
            expected_sha256: expected_sha256.clone(),
            observed_sha256: Some(expected_sha256.clone()),
            matched: true,
        })
        .collect();
    let mut inconclusive = false;

    for run in 1..=RUN_COUNT {
        let outcome = match run_exact_child(&mara, &fixture) {
            Ok(outcome) => outcome,
            Err(_) => {
                inconclusive = true;
                persist_unavailable_records(
                    &evidence,
                    &mut records,
                    run as u8,
                    "capture_unavailable",
                )?;
                break;
            }
        };
        write_bytes(
            &evidence.join(format!("run-{run:02}.stdout.json")),
            &outcome.stdout,
        )?;
        write_bytes(
            &evidence.join(format!("run-{run:02}.stderr")),
            &outcome.stderr,
        )?;

        let revalidation = revalidate_fixture(&fixture, &manifest, &pin);
        let mut record = measurement_record(run as u8, outcome, &revalidation);
        match revalidation {
            Ok(snapshot) => {
                fixture_files = snapshot.files.clone();
                write_json(
                    &evidence.join(format!("run-{run:02}.measurement.json")),
                    &record,
                )?;
                records.push(record);
                if !snapshot.integrity_matches {
                    for withheld in run + 1..=RUN_COUNT {
                        let withheld_record =
                            withheld_record(withheld as u8, "fixture_integrity_changed");
                        write_json(
                            &evidence.join(format!("run-{withheld:02}.measurement.json")),
                            &withheld_record,
                        )?;
                        records.push(withheld_record);
                    }
                    break;
                }
            }
            Err(_) => {
                inconclusive = true;
                record.fixture_verified = None;
                record.passed = false;
                record.error = Some("fixture_revalidation_unavailable".into());
                write_json(
                    &evidence.join(format!("run-{run:02}.measurement.json")),
                    &record,
                )?;
                records.push(record);
                for withheld in run + 1..=RUN_COUNT {
                    let withheld_record =
                        withheld_record(withheld as u8, "fixture_revalidation_unavailable");
                    write_json(
                        &evidence.join(format!("run-{withheld:02}.measurement.json")),
                        &withheld_record,
                    )?;
                    records.push(withheld_record);
                }
                break;
            }
        }
    }

    let passed = records.len() == RUN_COUNT && records.iter().all(|record| record.passed);
    let result = if inconclusive {
        "inconclusive"
    } else if passed {
        "passed"
    } else {
        "failed"
    };
    write_summary(
        &evidence,
        SummaryInputs {
            source_commit,
            xtask_sha256,
            mara_sha256,
            expected_manifest_sha256,
            fixture_files,
            records,
            result,
        },
    )?;
    if passed {
        Ok(())
    } else {
        Err(ToolError("scale qualification did not pass".into()))
    }
}

fn write_fixture(fixture: &Path) -> Result<()> {
    let mara_dir = fixture.join(".mara");
    fs::create_dir(&mara_dir)
        .map_err(|error| ToolError(format!("cannot create fixture .mara directory: {error}")))?;
    write_new_file(&mara_dir.join("project.toml"), PROJECT_TOML.as_bytes())?;
    write_new_file(&mara_dir.join("schema.yaml"), SCHEMA_YAML.as_bytes())?;
    for document in 0..DOCUMENT_COUNT {
        let start = document * (ITEM_COUNT / DOCUMENT_COUNT);
        let end = start + (ITEM_COUNT / DOCUMENT_COUNT);
        let mut contents = String::new();
        for item in start..end {
            if item != start {
                contents.push('\n');
            }
            contents.push_str(&item_block(item));
        }
        let path = fixture.join(format!("items-{document:03}.mara.md"));
        write_new_file(&path, contents.as_bytes())?;
    }
    Ok(())
}

fn item_block(item: usize) -> String {
    let mut block = format!(
        ":::scale {}\n:id: SCALE-{item:05}\n:title: Scale item {item:05}\n",
        scale_mid(item)
    );
    for delta in 1..=EDGES_PER_ITEM {
        block.push_str(&format!(
            ":depends_on: SCALE-{:05}\n",
            (item + delta) % ITEM_COUNT
        ));
    }
    block.push_str("\n:::\n");
    block
}

fn scale_mid(item: usize) -> String {
    const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    let mut value = (item + 1) as u64;
    let mut encoded = [b'0'; 26];
    for digit in encoded.iter_mut().rev() {
        *digit = CROCKFORD[(value & 31) as usize];
        value >>= 5;
    }
    format!(
        "m_{}",
        std::str::from_utf8(&encoded).expect("Crockford alphabet is UTF-8")
    )
}

fn source_repository() -> Result<PathBuf> {
    if env::var_os("CARGO_TARGET_DIR").is_some() {
        return Err(ToolError(
            "CARGO_TARGET_DIR must be unset for qualification commands".into(),
        ));
    }
    let cwd = fs::canonicalize(
        env::current_dir()
            .map_err(|error| ToolError(format!("cannot determine current directory: {error}")))?,
    )
    .map_err(|error| ToolError(format!("cannot canonicalize current directory: {error}")))?;
    let root_text = git_output(&cwd, ["rev-parse", "--show-toplevel"])?;
    let root = fs::canonicalize(root_text).map_err(|error| {
        ToolError(format!(
            "cannot canonicalize source repository root: {error}"
        ))
    })?;
    if cwd != root {
        return Err(ToolError(
            "qualification commands must be invoked from the canonical source repository root"
                .into(),
        ));
    }
    Ok(root)
}

fn canonical_xtask_executable(source_repo: &Path) -> Result<PathBuf> {
    let current = fs::canonicalize(env::current_exe().map_err(|error| {
        ToolError(format!(
            "cannot find running mara-xtask executable: {error}"
        ))
    })?)
    .map_err(|error| {
        ToolError(format!(
            "cannot canonicalize running mara-xtask executable: {error}"
        ))
    })?;
    let expected = source_repo.join("target/debug/mara-xtask");
    require_regular_file(&expected, "target/debug/mara-xtask")?;
    let expected = fs::canonicalize(&expected).map_err(|error| {
        ToolError(format!(
            "cannot canonicalize target/debug/mara-xtask: {error}"
        ))
    })?;
    require_exact_xtask_executable(&current, &expected)
}

fn require_exact_xtask_executable(current: &Path, expected: &Path) -> Result<PathBuf> {
    require_regular_file(expected, "target/debug/mara-xtask")?;
    if current != expected {
        return Err(ToolError(
            "qualification commands must use the canonical target/debug/mara-xtask executable"
                .into(),
        ));
    }
    Ok(expected.to_path_buf())
}

struct CandidateRoot {
    root: PathBuf,
    parent: PathBuf,
}

fn candidate_root(argument: &Path, must_exist: bool) -> Result<CandidateRoot> {
    if !argument.is_absolute() {
        return Err(ToolError("--qualification-root must be absolute".into()));
    }
    let parent_argument = argument
        .parent()
        .filter(|parent| *parent != argument)
        .ok_or_else(|| ToolError("--qualification-root must name a final path component".into()))?;
    let final_component = argument
        .file_name()
        .filter(|component| !component.is_empty())
        .ok_or_else(|| ToolError("--qualification-root must name a final path component".into()))?;
    let parent = fs::canonicalize(parent_argument).map_err(|error| {
        ToolError(format!(
            "cannot canonicalize qualification-root parent {}: {error}",
            parent_argument.display()
        ))
    })?;
    let root = parent.join(final_component);
    match fs::symlink_metadata(&root) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(ToolError(
                "qualification-root final component must not be a symlink".into(),
            ));
        }
        Ok(_) if !must_exist => {
            return Err(ToolError(
                "generate-scale-v01 requires a nonexistent qualification root".into(),
            ));
        }
        Ok(metadata) if must_exist && !metadata.is_dir() => {
            return Err(ToolError(
                "qualification-root must be an existing directory".into(),
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound && must_exist => {
            return Err(ToolError(
                "measure-scale-v01 requires an existing qualification root".into(),
            ));
        }
        Err(error) if error.kind() != io::ErrorKind::NotFound => {
            return Err(ToolError(format!(
                "cannot inspect qualification root: {error}"
            )));
        }
        _ => {}
    }
    let root = if must_exist {
        fs::canonicalize(&root).map_err(|error| {
            ToolError(format!("cannot canonicalize qualification root: {error}"))
        })?
    } else {
        root
    };
    Ok(CandidateRoot { root, parent })
}

fn ensure_isolated_root(root: &Path, source_repo: &Path, candidate_parent: &Path) -> Result<()> {
    if root.starts_with(source_repo) {
        return Err(ToolError(
            "qualification root must be outside the source repository".into(),
        ));
    }
    let tmp = fs::canonicalize("/tmp").unwrap_or_else(|_| PathBuf::from("/tmp"));
    if root.starts_with(&tmp) {
        return Err(ToolError(
            "qualification root must be outside physically resolved /tmp".into(),
        ));
    }
    for worktree in git_worktrees(source_repo)? {
        if root.starts_with(&worktree) {
            return Err(ToolError(format!(
                "qualification root must be outside Git worktree {}",
                worktree.display()
            )));
        }
    }
    if git_command(candidate_parent, ["rev-parse", "--show-toplevel"]).is_ok() {
        return Err(ToolError(
            "qualification-root parent is inside a Git worktree".into(),
        ));
    }
    Ok(())
}

fn ensure_root_has_no_git_entry(root: &Path) -> Result<()> {
    match fs::symlink_metadata(root.join(".git")) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Err(ToolError("qualification root must not contain .git".into())),
        Err(error) => Err(ToolError(format!(
            "cannot inspect qualification root .git entry: {error}"
        ))),
    }
}

fn path_is_in_source_control(root: &Path, source_repo: &Path) -> Result<bool> {
    if root.starts_with(source_repo) {
        return Ok(true);
    }
    for worktree in git_worktrees(source_repo)? {
        if root.starts_with(&worktree) {
            return Ok(true);
        }
    }
    Ok(git_command(
        root.parent()
            .ok_or_else(|| ToolError("qualification root has no parent".into()))?,
        ["rev-parse", "--show-toplevel"],
    )
    .is_ok())
}

fn git_worktrees(source_repo: &Path) -> Result<Vec<PathBuf>> {
    let output = git_output(source_repo, ["worktree", "list", "--porcelain"])?;
    output
        .lines()
        .filter_map(|line| line.strip_prefix("worktree "))
        .map(|path| {
            fs::canonicalize(path).map_err(|error| {
                ToolError(format!("cannot canonicalize listed Git worktree: {error}"))
            })
        })
        .collect()
}

fn git_output<const N: usize>(directory: &Path, arguments: [&str; N]) -> Result<String> {
    let output = git_command(directory, arguments)?;
    Ok(String::from_utf8(output.stdout)
        .map_err(|error| ToolError(format!("Git emitted non-UTF-8 output: {error}")))?
        .trim()
        .to_owned())
}

fn git_command<const N: usize>(
    directory: &Path,
    arguments: [&str; N],
) -> Result<std::process::Output> {
    let output = Command::new("git")
        .arg("-C")
        .arg(directory)
        .args(arguments)
        .output()
        .map_err(|error| ToolError(format!("cannot invoke git: {error}")))?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(ToolError(format!(
            "git command failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}

fn storage_info(path: &Path) -> Result<StorageInfo> {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::ffi::OsStrExt;
        let c_path = CString::new(path.as_os_str().as_bytes())
            .map_err(|_| ToolError("storage path contains an interior NUL".into()))?;
        let mut statfs = unsafe { std::mem::zeroed::<libc::statfs>() };
        let mut statvfs = unsafe { std::mem::zeroed::<libc::statvfs>() };
        if unsafe { libc::statfs(c_path.as_ptr(), &mut statfs) } != 0 {
            return Err(ToolError(format!(
                "statfs failed: {}",
                io::Error::last_os_error()
            )));
        }
        if unsafe { libc::statvfs(c_path.as_ptr(), &mut statvfs) } != 0 {
            return Err(ToolError(format!(
                "statvfs failed: {}",
                io::Error::last_os_error()
            )));
        }
        let block_size = statvfs.f_frsize as u64;
        let filesystem_type = format!("{:x}", statfs.f_type as u64);
        let filesystem_magic = statfs.f_type as u64;
        Ok(StorageInfo {
            filesystem_type,
            total_bytes: statvfs.f_blocks as u64 * block_size,
            available_bytes: statvfs.f_bavail as u64 * block_size,
            on_tmpfs_or_ramfs: matches!(filesystem_magic, 0x0102_1994 | 0x8584_58f6),
        })
    }
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::ffi::OsStrExt;
        let c_path = CString::new(path.as_os_str().as_bytes())
            .map_err(|_| ToolError("storage path contains an interior NUL".into()))?;
        let mut statfs = unsafe { std::mem::zeroed::<libc::statfs>() };
        let mut statvfs = unsafe { std::mem::zeroed::<libc::statvfs>() };
        if unsafe { libc::statfs(c_path.as_ptr(), &mut statfs) } != 0 {
            return Err(ToolError(format!(
                "statfs failed: {}",
                io::Error::last_os_error()
            )));
        }
        if unsafe { libc::statvfs(c_path.as_ptr(), &mut statvfs) } != 0 {
            return Err(ToolError(format!(
                "statvfs failed: {}",
                io::Error::last_os_error()
            )));
        }
        let filesystem_type = unsafe { std::ffi::CStr::from_ptr(statfs.f_fstypename.as_ptr()) }
            .to_string_lossy()
            .to_lowercase();
        let block_size = statvfs.f_frsize as u64;
        return Ok(StorageInfo {
            on_tmpfs_or_ramfs: matches!(filesystem_type.as_str(), "tmpfs" | "ramfs"),
            filesystem_type,
            total_bytes: statvfs.f_blocks as u64 * block_size,
            available_bytes: statvfs.f_bavail as u64 * block_size,
        });
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = path;
        Err(ToolError(
            "qualification storage checks are supported only on Linux and macOS".into(),
        ))
    }
}

fn emit_precondition(
    source_repo: Option<&Path>,
    qualification_parent: Option<&Path>,
    qualification_root: Option<&Path>,
    storage: Option<&StorageInfo>,
    inside_source_control: bool,
    error: &str,
) -> Result<()> {
    let source_repo = source_repo.map(path_text);
    let qualification_parent = qualification_parent.map(path_text);
    let qualification_root = qualification_root.map(path_text);
    let record = PreconditionRecord {
        format: "mara.qualification.scale-v01.precondition",
        version: 1,
        source_repo: source_repo.as_deref(),
        qualification_parent: qualification_parent.as_deref(),
        qualification_root: qualification_root.as_deref(),
        filesystem_type: storage.map(|value| value.filesystem_type.as_str()),
        total_bytes: storage.map(|value| value.total_bytes),
        available_bytes: storage.map(|value| value.available_bytes),
        minimum_available_bytes: MINIMUM_AVAILABLE_BYTES,
        on_tmpfs_or_ramfs: storage.map(|value| value.on_tmpfs_or_ramfs),
        inside_source_control: Some(inside_source_control),
        error,
    };
    let mut output = serde_json::to_vec(&record)
        .map_err(|error| ToolError(format!("cannot serialize precondition record: {error}")))?;
    output.push(b'\n');
    let mut stdout = io::stdout();
    stdout
        .write_all(&output)
        .map_err(|error| ToolError(format!("cannot write precondition record: {error}")))
}

fn write_new_file(path: &Path, contents: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| ToolError(format!("cannot create {}: {error}", path.display())))?;
    file.write_all(contents)
        .map_err(|error| ToolError(format!("cannot write {}: {error}", path.display())))?;
    file.sync_all()
        .map_err(|error| ToolError(format!("cannot sync {}: {error}", path.display())))
}

fn write_bytes(path: &Path, contents: &[u8]) -> Result<()> {
    write_new_file(path, contents)
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let mut rendered = serde_json::to_vec_pretty(value)
        .map_err(|error| ToolError(format!("cannot serialize {}: {error}", path.display())))?;
    rendered.push(b'\n');
    write_bytes(path, &rendered)
}

fn hash_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)
        .map_err(|error| ToolError(format!("cannot open {}: {error}", path.display())))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| ToolError(format!("cannot read {}: {error}", path.display())))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn require_real_directory(path: &Path, label: &str) -> Result<()> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| ToolError(format!("cannot inspect {label}: {error}")))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(ToolError(format!("{label} must be a real directory")));
    }
    Ok(())
}

fn require_regular_file(path: &Path, label: &str) -> Result<()> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| ToolError(format!("cannot inspect {label}: {error}")))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(ToolError(format!(
            "{label} must be a regular non-symlink file"
        )));
    }
    Ok(())
}

fn ensure_fixture_has_no_git_context(fixture: &Path) -> Result<()> {
    if fixture.join(".git").exists() {
        return Err(ToolError("fixture must not contain .git".into()));
    }
    if git_command(fixture, ["rev-parse", "--show-toplevel"]).is_ok() {
        return Err(ToolError("fixture unexpectedly has Git context".into()));
    }
    Ok(())
}

fn parse_manifest_pin(path: &Path) -> Result<ManifestPin> {
    let contents = fs::read_to_string(path)
        .map_err(|error| ToolError(format!("cannot read oracle manifest record: {error}")))?;
    let mut sha256 = None;
    let mut entries = Vec::new();
    for line in contents.lines() {
        if let Some(value) = line.strip_prefix("manifest_sha256=") {
            sha256 = Some(value.to_owned());
        }
        if let Some(value) = line.strip_prefix("manifest_entry=") {
            let (path, digest) = value
                .split_once(' ')
                .ok_or_else(|| ToolError("oracle manifest record has invalid entry".into()))?;
            entries.push((path.to_owned(), digest.to_owned()));
        }
    }
    let sha256 =
        sha256.ok_or_else(|| ToolError("oracle manifest record lacks digest pin".into()))?;
    if !is_lowercase_sha256(&sha256) || entries.len() != 12 {
        return Err(ToolError("oracle manifest record is incomplete".into()));
    }
    if entries
        .iter()
        .any(|(_, digest)| !is_lowercase_sha256(digest))
    {
        return Err(ToolError(
            "oracle manifest record has invalid digest".into(),
        ));
    }
    Ok(ManifestPin { sha256, entries })
}

fn is_lowercase_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn unavailable_record(run: u8, error: &str) -> MeasurementRecord {
    MeasurementRecord {
        run,
        elapsed_ns: None,
        peak_rss_kib: None,
        exit_code: None,
        term_signal: None,
        timed_out: false,
        stdout_sha256: None,
        stderr_sha256: None,
        mara_status: None,
        diagnostic_count: None,
        documents: None,
        items: None,
        edges: None,
        fixture_verified: None,
        passed: false,
        error: Some(error.into()),
    }
}

fn withheld_record(run: u8, error: &str) -> MeasurementRecord {
    unavailable_record(run, error)
}

fn persist_unavailable_records(
    evidence: &Path,
    records: &mut Vec<MeasurementRecord>,
    first_run: u8,
    error: &str,
) -> Result<()> {
    for run in first_run..=RUN_COUNT as u8 {
        let record = unavailable_record(run, error);
        write_json(
            &evidence.join(format!("run-{run:02}.measurement.json")),
            &record,
        )?;
        records.push(record);
    }
    Ok(())
}

#[cfg(target_os = "linux")]
struct ChildOutcome {
    elapsed_ns: u64,
    peak_rss_kib: u64,
    exit_code: Option<i32>,
    term_signal: Option<i32>,
    timed_out: bool,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

#[cfg(target_os = "linux")]
fn terminate_and_reap_child(
    pid: libc::pid_t,
    status: &mut libc::c_int,
    usage: &mut libc::rusage,
) -> Result<()> {
    if unsafe { libc::kill(-pid, libc::SIGKILL) } != 0 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(libc::ESRCH) {
            return Err(ToolError(format!(
                "cannot terminate child process group: {error}"
            )));
        }
    }
    loop {
        let waited = unsafe { libc::wait4(pid, status, 0, usage) };
        if waited == pid {
            return Ok(());
        }
        if waited == -1 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            if error.raw_os_error() == Some(libc::ECHILD) {
                return Ok(());
            }
            return Err(ToolError(format!(
                "cannot reap child process group: {error}"
            )));
        }
    }
}

#[cfg(target_os = "linux")]
fn run_exact_child(mara: &Path, fixture: &Path) -> Result<ChildOutcome> {
    run_exact_child_with_limit(
        mara,
        fixture,
        &["check", "--format", "json"],
        Duration::from_nanos(ELAPSED_LIMIT_NS),
    )
}

#[cfg(target_os = "linux")]
fn run_exact_child_with_limit(
    program: &Path,
    directory: &Path,
    arguments: &[&str],
    limit: Duration,
) -> Result<ChildOutcome> {
    use std::os::unix::process::CommandExt;

    let mut command = Command::new(program);
    command
        .current_dir(directory)
        .args(arguments)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    unsafe {
        command.pre_exec(|| {
            if libc::setpgid(0, 0) != 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }

    let started = Instant::now();
    let mut child = command
        .spawn()
        .map_err(|error| ToolError(format!("cannot spawn Mara child: {error}")))?;
    let pid = child.id() as libc::pid_t;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| ToolError("child stdout was not piped".into()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| ToolError("child stderr was not piped".into()))?;
    let stdout_reader = std::thread::spawn(move || read_pipe(stdout));
    let stderr_reader = std::thread::spawn(move || read_pipe(stderr));

    let deadline = started + limit;
    let mut status = 0;
    let mut usage = unsafe { std::mem::zeroed::<libc::rusage>() };
    let mut timed_out = false;
    loop {
        let waited = unsafe { libc::wait4(pid, &mut status, libc::WNOHANG, &mut usage) };
        if waited == pid {
            break;
        }
        if waited == -1 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            if let Err(cleanup_error) = terminate_and_reap_child(pid, &mut status, &mut usage) {
                return Err(ToolError(format!(
                    "cannot wait for exact child: {error}; {cleanup_error}"
                )));
            }
            return Err(ToolError(format!("cannot wait for exact child: {error}")));
        }
        if Instant::now() >= deadline {
            timed_out = true;
            terminate_and_reap_child(pid, &mut status, &mut usage)
                .map_err(|error| ToolError(format!("cannot clean up timed-out child: {error}")))?;
            break;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        std::thread::sleep(remaining.min(Duration::from_millis(10)));
    }
    let elapsed_ns = started.elapsed().as_nanos().min(u64::MAX as u128) as u64;
    let stdout = stdout_reader
        .join()
        .map_err(|_| ToolError("stdout capture thread panicked".into()))??;
    let stderr = stderr_reader
        .join()
        .map_err(|_| ToolError("stderr capture thread panicked".into()))??;
    let signal = status & 0x7f;
    let (exit_code, term_signal) = if signal == 0 {
        (Some((status >> 8) & 0xff), None)
    } else {
        (None, Some(signal))
    };
    Ok(ChildOutcome {
        elapsed_ns,
        peak_rss_kib: usage.ru_maxrss as u64,
        exit_code,
        term_signal,
        timed_out,
        stdout,
        stderr,
    })
}

#[cfg(target_os = "linux")]
fn read_pipe(mut pipe: impl Read) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    pipe.read_to_end(&mut bytes)
        .map_err(|error| ToolError(format!("cannot capture child stream: {error}")))?;
    Ok(bytes)
}

#[cfg(target_os = "linux")]
fn revalidate_fixture(
    fixture: &Path,
    manifest: &Path,
    pin: &ManifestPin,
) -> Result<FixtureRevalidation> {
    if hash_file(manifest)? != pin.sha256 {
        return Ok(FixtureRevalidation {
            files: fixture_mismatch_snapshot(fixture, pin)?,
            integrity_matches: false,
        });
    }
    match fs::symlink_metadata(fixture) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
        Ok(_) => {
            return Ok(FixtureRevalidation {
                files: fixture_mismatch_snapshot(fixture, pin)?,
                integrity_matches: false,
            });
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(FixtureRevalidation {
                files: fixture_mismatch_snapshot(fixture, pin)?,
                integrity_matches: false,
            });
        }
        Err(error) => {
            return Err(ToolError(format!(
                "cannot inspect fixture directory: {error}"
            )));
        }
    }
    match fs::symlink_metadata(fixture.join(".mara")) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
        Ok(_) => {
            return Ok(FixtureRevalidation {
                files: fixture_mismatch_snapshot(fixture, pin)?,
                integrity_matches: false,
            });
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(FixtureRevalidation {
                files: fixture_mismatch_snapshot(fixture, pin)?,
                integrity_matches: false,
            });
        }
        Err(error) => {
            return Err(ToolError(format!(
                "cannot inspect fixture .mara directory: {error}"
            )));
        }
    }
    let expected_names = pin
        .entries
        .iter()
        .map(|(path, _)| path.clone())
        .chain(std::iter::once(".mara".to_owned()))
        .collect::<BTreeSet<_>>();
    let actual_names = fixture_entry_set(fixture)?;
    if actual_names != expected_names {
        return Ok(FixtureRevalidation {
            files: fixture_mismatch_snapshot(fixture, pin)?,
            integrity_matches: false,
        });
    }
    let mut entries = Vec::new();
    for (path, expected_sha256) in &pin.entries {
        let full_path = fixture.join(path);
        let observed_sha256 = file_digest_if_regular(&full_path)?;
        let matched = observed_sha256.as_deref() == Some(expected_sha256);
        entries.push(FixtureFile {
            path: path.clone(),
            expected_sha256: expected_sha256.clone(),
            observed_sha256,
            matched,
        });
    }
    Ok(FixtureRevalidation {
        integrity_matches: entries.iter().all(|entry| entry.matched),
        files: entries,
    })
}

#[cfg(target_os = "linux")]
fn fixture_mismatch_snapshot(fixture: &Path, pin: &ManifestPin) -> Result<Vec<FixtureFile>> {
    let mut entries = Vec::new();
    for (path, expected_sha256) in &pin.entries {
        let observed_sha256 = file_digest_if_regular(&fixture.join(path))?;
        entries.push(FixtureFile {
            path: path.clone(),
            expected_sha256: expected_sha256.clone(),
            observed_sha256: observed_sha256.clone(),
            matched: observed_sha256.as_deref() == Some(expected_sha256),
        });
    }
    Ok(entries)
}

#[cfg(target_os = "linux")]
fn fixture_entry_set(fixture: &Path) -> Result<BTreeSet<String>> {
    let mut entries = BTreeSet::new();
    for entry in fs::read_dir(fixture)
        .map_err(|error| ToolError(format!("cannot read fixture directory: {error}")))?
    {
        let entry =
            entry.map_err(|error| ToolError(format!("cannot read fixture entry: {error}")))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        entries.insert(name.clone());
        if name == ".mara" {
            for nested in fs::read_dir(entry.path()).map_err(|error| {
                ToolError(format!("cannot read fixture .mara directory: {error}"))
            })? {
                let nested = nested.map_err(|error| {
                    ToolError(format!("cannot read fixture .mara entry: {error}"))
                })?;
                entries.insert(format!(".mara/{}", nested.file_name().to_string_lossy()));
            }
        }
    }
    Ok(entries)
}

#[cfg(target_os = "linux")]
fn file_digest_if_regular(path: &Path) -> Result<Option<String>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => Ok(None),
        Ok(_) => hash_file(path).map(Some),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(ToolError(format!(
            "cannot inspect fixture file {}: {error}",
            path.display()
        ))),
    }
}

#[cfg(target_os = "linux")]
fn measurement_record(
    run: u8,
    outcome: ChildOutcome,
    revalidation: &Result<FixtureRevalidation>,
) -> MeasurementRecord {
    let parsed = serde_json::from_slice::<serde_json::Value>(&outcome.stdout).ok();
    let status = parsed
        .as_ref()
        .and_then(|value| value.get("status"))
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned);
    let diagnostic_count = parsed
        .as_ref()
        .and_then(|value| value.get("diagnostics"))
        .and_then(serde_json::Value::as_array)
        .map(|diagnostics| diagnostics.len() as u64);
    let summary = parsed
        .as_ref()
        .and_then(|value| value.get("data"))
        .and_then(|value| value.get("summary"));
    let documents = summary
        .and_then(|value| value.get("documents"))
        .and_then(serde_json::Value::as_u64);
    let items = summary
        .and_then(|value| value.get("items"))
        .and_then(serde_json::Value::as_u64);
    let edges = summary
        .and_then(|value| value.get("edges"))
        .and_then(serde_json::Value::as_u64);
    let fixture_verified = revalidation
        .as_ref()
        .ok()
        .map(|snapshot| snapshot.integrity_matches);
    let passed = outcome.exit_code == Some(0)
        && status.as_deref() == Some("ok")
        && diagnostic_count == Some(0)
        && documents == Some(10)
        && items == Some(10_000)
        && edges == Some(100_000)
        && !outcome.timed_out
        && outcome.elapsed_ns <= ELAPSED_LIMIT_NS
        && outcome.peak_rss_kib <= PEAK_RSS_LIMIT_KIB
        && fixture_verified == Some(true);
    let error = if revalidation.is_err() {
        Some("fixture_revalidation_unavailable".into())
    } else if fixture_verified == Some(false) {
        Some("fixture_integrity_changed".into())
    } else {
        None
    };
    MeasurementRecord {
        run,
        elapsed_ns: Some(outcome.elapsed_ns),
        peak_rss_kib: Some(outcome.peak_rss_kib),
        exit_code: outcome.exit_code,
        term_signal: outcome.term_signal,
        timed_out: outcome.timed_out,
        stdout_sha256: Some(hash_bytes(&outcome.stdout)),
        stderr_sha256: Some(hash_bytes(&outcome.stderr)),
        mara_status: status,
        diagnostic_count,
        documents,
        items,
        edges,
        fixture_verified,
        passed,
        error,
    }
}

fn write_summary(evidence: &Path, inputs: SummaryInputs) -> Result<()> {
    let max_elapsed_ns = inputs
        .records
        .iter()
        .filter_map(|record| record.elapsed_ns)
        .max();
    let max_peak_rss_kib = inputs
        .records
        .iter()
        .filter_map(|record| record.peak_rss_kib)
        .max();
    write_json(
        &evidence.join("qualification-summary.json"),
        &QualificationSummary {
            format: "mara.qualification.scale-v01",
            version: 1,
            source_commit: inputs.source_commit,
            xtask_sha256: inputs.xtask_sha256,
            mara_sha256: inputs.mara_sha256,
            expected_manifest_sha256: inputs.expected_manifest_sha256,
            fixture_files: inputs.fixture_files,
            runs: inputs.records,
            max_elapsed_ns,
            max_peak_rss_kib,
            elapsed_limit_ns: ELAPSED_LIMIT_NS,
            peak_rss_limit_kib: PEAK_RSS_LIMIT_KIB,
            result: inputs.result,
        },
    )
}

#[cfg(test)]
fn manifest_paths() -> Vec<String> {
    let mut paths = vec![".mara/project.toml".into(), ".mara/schema.yaml".into()];
    paths.extend((0..DOCUMENT_COUNT).map(|index| format!("items-{index:03}.mara.md")));
    paths
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn fixture_is_deterministic_and_has_the_required_topology() {
        let temporary = tempfile::tempdir().expect("temporary root");
        let first = temporary.path().join("first");
        let second = temporary.path().join("second");
        fs::create_dir(&first).expect("first fixture directory");
        fs::create_dir(&second).expect("second fixture directory");
        write_fixture(&first).expect("write first fixture");
        write_fixture(&second).expect("write second fixture");

        let first_files = fixture_digests(&first);
        assert_eq!(first_files, fixture_digests(&second));
        assert_eq!(first_files.len(), 12);
        let contents =
            fs::read_to_string(first.join("items-000.mara.md")).expect("first item document");
        assert!(contents.starts_with(":::scale m_00000000000000000000000001\n:id: SCALE-00000"));
        assert!(contents.contains(":depends_on: SCALE-00010\n\n:::\n"));
    }

    #[cfg(unix)]
    #[test]
    fn evidence_outputs_are_created_once_without_following_symlinks() {
        use std::os::unix::fs::symlink;

        let temporary = tempfile::tempdir().expect("temporary root");
        let output = temporary.path().join("evidence.json");
        write_bytes(&output, b"first").expect("initial output");
        assert!(write_bytes(&output, b"second").is_err());
        assert_eq!(fs::read(&output).expect("initial bytes"), b"first");

        let target = temporary.path().join("outside-evidence.json");
        fs::write(&target, b"outside").expect("outside target");
        let redirected_output = temporary.path().join("redirected-evidence.json");
        symlink(&target, &redirected_output).expect("redirected output");
        assert!(write_bytes(&redirected_output, b"replacement").is_err());
        assert_eq!(fs::read(&target).expect("outside bytes"), b"outside");
    }

    #[test]
    fn command_surface_rejects_unknown_and_relative_roots() {
        assert!(
            Cli::try_parse_from([
                "mara-xtask",
                "qualification",
                "generate-scale-v01",
                "--qualification-root",
                "/qualification",
                "--unknown",
            ])
            .is_err()
        );
        assert!(candidate_root(Path::new("relative"), false).is_err());
    }

    #[test]
    fn qualification_xtask_must_be_the_canonical_debug_executable() {
        let temporary = tempfile::tempdir().expect("temporary root");
        let expected = temporary.path().join("target/debug/mara-xtask");
        let copied = temporary.path().join("copied-mara-xtask");
        fs::create_dir_all(expected.parent().expect("target directory")).expect("target directory");
        fs::write(&expected, "canonical").expect("canonical executable");
        fs::write(&copied, "canonical").expect("copied executable");
        assert_eq!(
            require_exact_xtask_executable(&expected, &expected).expect("canonical executable"),
            expected
        );
        assert!(require_exact_xtask_executable(&copied, &expected).is_err());
    }

    #[test]
    fn storage_capacity_and_tmpfs_are_rejected() {
        let low_capacity = StorageInfo {
            filesystem_type: "ext4".into(),
            total_bytes: MINIMUM_AVAILABLE_BYTES,
            available_bytes: MINIMUM_AVAILABLE_BYTES - 1,
            on_tmpfs_or_ramfs: false,
        };
        assert_eq!(
            storage_precondition_error(&low_capacity, false),
            Some("insufficient_capacity")
        );
        let volatile = StorageInfo {
            on_tmpfs_or_ramfs: true,
            ..low_capacity
        };
        assert_eq!(
            storage_precondition_error(&volatile, false),
            Some("tmpfs_or_ramfs")
        );
        assert_eq!(
            storage_precondition_error(&volatile, true),
            Some("inside_source_control")
        );
    }

    #[test]
    fn measurement_requires_a_valid_generation_storage_record() {
        let temporary = tempfile::tempdir().expect("temporary root");
        let source_repo = temporary.path().join("source");
        let qualification_parent = temporary.path().join("external");
        let qualification_root = qualification_parent.join("qualification");
        let evidence = qualification_root.join("evidence");
        fs::create_dir(&source_repo).expect("source repository");
        fs::create_dir(&qualification_parent).expect("qualification parent");
        fs::create_dir(&qualification_root).expect("qualification root");
        fs::create_dir(&evidence).expect("evidence directory");

        assert!(
            validate_generation_storage_record(
                &evidence,
                &source_repo,
                &qualification_parent,
                &qualification_root,
            )
            .is_err()
        );

        let record_path = evidence.join("storage-before-generation.json");
        let valid = StorageRecord {
            source_repo: path_text(&source_repo),
            qualification_parent: path_text(&qualification_parent),
            qualification_root: path_text(&qualification_root),
            filesystem_type: "ext4".into(),
            total_bytes: MINIMUM_AVAILABLE_BYTES,
            available_bytes: MINIMUM_AVAILABLE_BYTES,
            minimum_available_bytes: MINIMUM_AVAILABLE_BYTES,
            on_tmpfs_or_ramfs: false,
            inside_source_control: false,
            passed: true,
        };
        fs::write(
            &record_path,
            serde_json::to_vec(&valid).expect("valid record"),
        )
        .expect("storage record");
        validate_generation_storage_record(
            &evidence,
            &source_repo,
            &qualification_parent,
            &qualification_root,
        )
        .expect("valid generation storage record");

        let mut unknown_field = serde_json::to_value(&valid).expect("record as JSON");
        unknown_field
            .as_object_mut()
            .expect("record object")
            .insert("unexpected".into(), serde_json::Value::Bool(true));
        fs::write(
            &record_path,
            serde_json::to_vec(&unknown_field).expect("record with unknown field"),
        )
        .expect("unknown-field storage record");
        assert!(
            validate_generation_storage_record(
                &evidence,
                &source_repo,
                &qualification_parent,
                &qualification_root,
            )
            .is_err()
        );

        let invalid = StorageRecord {
            passed: false,
            ..valid
        };
        fs::write(
            &record_path,
            serde_json::to_vec(&invalid).expect("invalid record"),
        )
        .expect("invalid storage record");
        assert!(
            validate_generation_storage_record(
                &evidence,
                &source_repo,
                &qualification_parent,
                &qualification_root,
            )
            .is_err()
        );
    }

    #[test]
    fn post_creation_gate_failure_removes_the_empty_qualification_root() {
        let temporary = tempfile::tempdir().expect("temporary root");
        let root = temporary.path().join("qualification-root");
        fs::create_dir(&root).expect("created qualification root");
        let error = rollback_created_root(&root, "storage gate failed".into());
        assert_eq!(error.to_string(), "storage gate failed");
        assert!(!root.exists());
        fs::create_dir(&root).expect("safe retry can recreate qualification root");
    }

    #[cfg(unix)]
    #[test]
    fn qualification_root_rejects_a_dangling_git_entry() {
        use std::os::unix::fs::symlink;

        let temporary = tempfile::tempdir().expect("temporary root");
        let root = temporary.path().join("qualification-root");
        fs::create_dir(&root).expect("qualification root");
        symlink(root.join("missing-git-directory"), root.join(".git"))
            .expect("dangling .git entry");
        assert!(ensure_root_has_no_git_entry(&root).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn a_created_root_must_remain_a_real_directory() {
        use std::os::unix::fs::symlink;

        let temporary = tempfile::tempdir().expect("temporary root");
        let real_root = temporary.path().join("real-root");
        fs::create_dir(&real_root).expect("real root");
        let candidate_root = temporary.path().join("qualification-root");
        symlink(&real_root, &candidate_root).expect("replaced root");
        assert!(require_real_directory(&candidate_root, "qualification root").is_err());
    }

    #[test]
    fn workspace_isolation_rejects_a_root_beneath_the_source_repository() {
        let source = fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
            .expect("source repository");
        assert!(ensure_isolated_root(&source.join("not-external"), &source, &source).is_err());
        let tmp = fs::canonicalize("/tmp").expect("canonical /tmp");
        assert!(ensure_isolated_root(&tmp.join("not-external"), &source, &tmp).is_err());
    }

    #[test]
    fn a_tmp_root_is_not_labeled_as_source_control() {
        let source = fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
            .expect("source repository");
        let tmp = fs::canonicalize("/tmp").expect("canonical /tmp");
        assert!(
            !path_is_in_source_control(
                &tmp.join("mara-qualification-not-source-controlled"),
                &source
            )
            .expect("source-control classification")
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn revalidation_detects_an_oracle_pinned_fixture_mismatch() {
        let temporary = tempfile::tempdir().expect("temporary root");
        let fixture = temporary.path().join("fixture");
        fs::create_dir(&fixture).expect("fixture directory");
        write_fixture(&fixture).expect("fixture");
        let entries = fixture_digests(&fixture);
        let pin = ManifestPin {
            sha256: "0".repeat(64),
            entries: entries.into_iter().collect(),
        };
        let manifest = temporary.path().join("manifest");
        fs::write(&manifest, "not the pinned manifest\n").expect("manifest");
        let snapshot = revalidate_fixture(&fixture, &manifest, &pin).expect("revalidation");
        assert!(!snapshot.integrity_matches);
        assert!(snapshot.files.iter().all(|entry| entry.matched));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn revalidation_rejects_a_symlinked_fixture_directory() {
        use std::os::unix::fs::symlink;

        let temporary = tempfile::tempdir().expect("temporary root");
        let real_fixture = temporary.path().join("real-fixture");
        fs::create_dir(&real_fixture).expect("real fixture directory");
        write_fixture(&real_fixture).expect("fixture");
        let fixture = temporary.path().join("fixture");
        symlink(&real_fixture, &fixture).expect("fixture symlink");
        let manifest = temporary.path().join("manifest");
        fs::write(&manifest, "pinned manifest\n").expect("manifest");
        let pin = ManifestPin {
            sha256: hash_file(&manifest).expect("manifest hash"),
            entries: fixture_digests(&real_fixture).into_iter().collect(),
        };
        let snapshot = revalidate_fixture(&fixture, &manifest, &pin).expect("revalidation");
        assert!(!snapshot.integrity_matches);
        assert!(snapshot.files.iter().all(|entry| entry.matched));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn exact_child_timeout_is_observable() {
        let temporary = tempfile::tempdir().expect("temporary root");
        let script = temporary.path().join("sleep-forever.sh");
        fs::write(&script, "#!/bin/sh\nsleep 1\n").expect("script");
        let mut permissions = fs::metadata(&script).expect("metadata").permissions();
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(0o755);
        fs::set_permissions(&script, permissions).expect("permissions");
        let outcome =
            run_exact_child_with_limit(&script, temporary.path(), &[], Duration::from_millis(10))
                .expect("child outcome");
        assert!(outcome.timed_out);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn child_capture_setup_failure_is_observable() {
        let temporary = tempfile::tempdir().expect("temporary root");
        assert!(
            run_exact_child_with_limit(
                &temporary.path().join("missing-mara"),
                temporary.path(),
                &[],
                Duration::from_millis(10),
            )
            .is_err()
        );
    }

    #[test]
    fn capture_failure_is_serialized_for_the_current_and_withheld_runs() {
        let temporary = tempfile::tempdir().expect("temporary root");
        let mut records = Vec::new();
        persist_unavailable_records(temporary.path(), &mut records, 3, "capture_unavailable")
            .expect("capture evidence");
        assert_eq!(records.len(), 3);
        assert_eq!(
            records.iter().map(|record| record.run).collect::<Vec<_>>(),
            vec![3, 4, 5]
        );
        let current = fs::read_to_string(temporary.path().join("run-03.measurement.json"))
            .expect("current run evidence");
        assert!(current.contains("capture_unavailable"));
        assert!(temporary.path().join("run-05.measurement.json").is_file());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn workflow_uploads_evidence_before_a_cleanup_failure_can_fail_the_job() {
        let workflow = include_str!("../../../.github/workflows/scale-fixture-verification.yml");
        let upload = workflow
            .find("- name: Upload fixture-verification evidence\n        if: always()")
            .expect("always-upload evidence step");
        let cleanup = workflow
            .find("- name: Remove disposable external workspace\n        if: always()")
            .expect("always-run cleanup step");
        assert!(upload < cleanup);
        assert!(workflow[cleanup..].contains("rm -rf -- \"$root\""));
        assert!(workflow.contains("test ! -L \"$root\""));
        assert!(workflow[cleanup..].contains("|| [ -L \"$root\" ]"));
        assert!(workflow[cleanup..].contains("qualification root remained after cleanup"));
        assert!(workflow.contains("/usr/sbin/diskutil info -plist \"$RUNNER_TEMP\""));
        assert!(!workflow.contains("macOS) filesystem_type=\"$(stat -f %T"));
    }

    #[cfg(unix)]
    #[test]
    fn verifier_rejects_a_manifest_without_a_final_newline_and_retains_digest_evidence() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let temporary = tempfile::tempdir().expect("temporary root");
        let repo = temporary.path().join("source-repository");
        let qualification_root = temporary.path().join("qualification-root");
        let verifier = repo.join("tests/qualification/verify-scale-v01.sh");
        let manifest = repo.join("tests/qualification/scale-v01.SHA256SUMS");
        let mara = repo.join("target/release/mara");
        fs::create_dir(&repo).expect("source repository");
        fs::create_dir_all(verifier.parent().expect("verifier parent")).expect("verifier parent");
        fs::create_dir_all(mara.parent().expect("mara parent")).expect("mara parent");
        Command::new("git")
            .arg("init")
            .arg("--quiet")
            .arg(&repo)
            .status()
            .expect("initialize source Git repository")
            .success()
            .then_some(())
            .expect("source Git repository initialized");

        let mut manifest_bytes =
            include_bytes!("../../../tests/qualification/scale-v01.SHA256SUMS").to_vec();
        assert_eq!(manifest_bytes.pop(), Some(b'\n'));
        fs::write(&manifest, manifest_bytes).expect("manifest without final newline");
        fs::write(
            &verifier,
            include_str!("../../../tests/qualification/verify-scale-v01.sh"),
        )
        .expect("verifier");
        fs::write(
            &mara,
            "#!/bin/sh\nif [ -n \"${MARA_RAN_MARKER:-}\" ]; then : > \"$MARA_RAN_MARKER\"; fi\ncat <<'JSON'\n{\n  \"format\": \"mara.command\",\n  \"version\": 1,\n  \"command\": \"check\",\n  \"status\": \"ok\",\n  \"project\": {\n    \"name\": \"mara-scale-v01\",\n    \"root\": \".\",\n    \"schema_name\": \"mara-scale-v01\",\n    \"schema_version\": \"0.1.0\",\n    \"schema_path\": \".mara/schema.yaml\"\n  },\n  \"diagnostics\": [],\n  \"data\": {\n    \"summary\": {\n      \"documents\": 10,\n      \"items\": 10000,\n      \"source_nodes\": 0,\n      \"edges\": 100000,\n      \"mentions\": 0,\n      \"external_nodes\": 0,\n      \"errors\": 0,\n      \"warnings\": 0,\n      \"info\": 0\n    }\n  },\n  \"error\": null\n}\nJSON\n",
        )
        .expect("Mara fixture command");
        for path in [&verifier, &mara] {
            let mut permissions = fs::metadata(path)
                .expect("executable metadata")
                .permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions).expect("executable permissions");
        }

        fs::create_dir(&qualification_root).expect("qualification root");
        let fixture = qualification_root.join("fixture");
        fs::create_dir(&fixture).expect("fixture directory");
        write_fixture(&fixture).expect("fixture");
        fs::create_dir(qualification_root.join("evidence")).expect("evidence directory");

        let status = Command::new("sh")
            .current_dir(&repo)
            .arg(&verifier)
            .arg("--qualification-root")
            .arg(&qualification_root)
            .status()
            .expect("run verifier");
        assert!(!status.success());

        let manifest_evidence =
            fs::read_to_string(qualification_root.join("evidence/manifest-path-check.txt"))
                .expect("manifest evidence");
        assert!(manifest_evidence.contains("manifest_error=missing_final_lf"));
        let digest_evidence =
            fs::read_to_string(qualification_root.join("evidence/fixture-sha256-check.txt"))
                .expect("digest evidence");
        assert_eq!(digest_evidence.matches("matched=true").count(), 12);
        assert!(digest_evidence.contains("expected_sha256="));
        assert!(digest_evidence.contains("observed_sha256="));
        let preflight_evidence =
            fs::read_to_string(qualification_root.join("evidence/preflight-check-exit.txt"))
                .expect("preflight evidence");
        assert!(preflight_evidence.contains("json_validation=1"));

        fs::write(
            &manifest,
            include_bytes!("../../../tests/qualification/scale-v01.SHA256SUMS"),
        )
        .expect("restore canonical manifest");

        let topology_root = temporary.path().join("malformed-topology-root");
        let topology_fixture = topology_root.join("fixture");
        fs::create_dir(&topology_root).expect("topology qualification root");
        fs::create_dir(&topology_fixture).expect("topology fixture directory");
        write_fixture(&topology_fixture).expect("topology fixture");
        fs::write(
            topology_fixture.join("items-000.mara.md"),
            format!(
                "{}:depends_on: SCALE-bad\\n",
                fs::read_to_string(topology_fixture.join("items-000.mara.md"))
                    .expect("topology item document")
            ),
        )
        .expect("malformed topology relation");
        fs::create_dir(topology_root.join("evidence")).expect("topology evidence directory");
        let status = Command::new("sh")
            .current_dir(&repo)
            .arg(&verifier)
            .arg("--qualification-root")
            .arg(&topology_root)
            .status()
            .expect("run malformed-topology verifier");
        assert!(!status.success());
        let topology_evidence =
            fs::read_to_string(topology_root.join("evidence/topology-check.txt"))
                .expect("topology evidence");
        assert!(topology_evidence.contains("topology_error=malformed_target"));

        let invalid_type_root = temporary.path().join("invalid-type-root");
        let invalid_type_fixture = invalid_type_root.join("fixture");
        fs::create_dir(&invalid_type_root).expect("invalid-type qualification root");
        fs::create_dir(&invalid_type_fixture).expect("invalid-type fixture directory");
        write_fixture(&invalid_type_fixture).expect("invalid-type fixture");
        let symlink_target = temporary.path().join("outside-fixture-item");
        fs::write(&symlink_target, "outside fixture").expect("symlink target");
        let invalid_item = invalid_type_fixture.join("items-000.mara.md");
        fs::remove_file(&invalid_item).expect("remove regular fixture item");
        symlink(&symlink_target, &invalid_item).expect("symlink fixture item");
        fs::create_dir(invalid_type_root.join("evidence"))
            .expect("invalid-type evidence directory");
        let mara_marker = temporary.path().join("mara-ran-marker");
        let status = Command::new("sh")
            .current_dir(&repo)
            .env("MARA_RAN_MARKER", &mara_marker)
            .arg(&verifier)
            .arg("--qualification-root")
            .arg(&invalid_type_root)
            .status()
            .expect("run invalid-type verifier");
        assert!(!status.success());
        assert!(!mara_marker.exists());
        let invalid_type_evidence =
            fs::read_to_string(invalid_type_root.join("evidence/file-type-check.txt"))
                .expect("invalid-type evidence");
        assert!(invalid_type_evidence.contains("invalid_type=items-000.mara.md"));
        assert!(
            !invalid_type_root
                .join("evidence/preflight-check.json")
                .exists()
        );

        let unsupported_root = temporary.path().join("unsupported-qualification-root");
        let unsupported_fixture = unsupported_root.join("fixture");
        fs::create_dir(&unsupported_root).expect("unsupported qualification root");
        fs::create_dir(&unsupported_fixture).expect("unsupported fixture directory");
        write_fixture(&unsupported_fixture).expect("unsupported fixture");
        fs::create_dir(unsupported_root.join("evidence")).expect("unsupported evidence directory");
        let fake_bin = temporary.path().join("fake-bin");
        fs::create_dir(&fake_bin).expect("fake executable directory");
        let fake_uname = fake_bin.join("uname");
        fs::write(&fake_uname, "#!/bin/sh\nprintf '%s\\n' Unsupported\n").expect("fake uname");
        let mut permissions = fs::metadata(&fake_uname)
            .expect("fake uname metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_uname, permissions).expect("fake uname permissions");
        let inherited_path = env::var_os("PATH").expect("PATH");
        let path = format!(
            "{}:{}",
            fake_bin.display(),
            inherited_path.to_string_lossy()
        );
        let status = Command::new("sh")
            .current_dir(&repo)
            .env("PATH", path)
            .arg(&verifier)
            .arg("--qualification-root")
            .arg(&unsupported_root)
            .status()
            .expect("run unsupported-host verifier");
        assert!(!status.success());
        let unsupported_evidence =
            fs::read_to_string(unsupported_root.join("evidence/fixture-sha256-check.txt"))
                .expect("unsupported-host evidence");
        assert!(unsupported_evidence.contains("host=unsupported"));
    }

    fn fixture_digests(root: &Path) -> BTreeMap<String, String> {
        let mut values = BTreeMap::new();
        for path in manifest_paths() {
            values.insert(path.clone(), hash_file(&root.join(path)).expect("digest"));
        }
        values
    }
}
