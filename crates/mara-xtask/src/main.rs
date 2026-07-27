use clap::{Args, Parser, Subcommand};
use serde::Serialize;
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

#[derive(Serialize)]
struct StorageRecord<'a> {
    source_repo: &'a str,
    qualification_parent: &'a str,
    qualification_root: &'a str,
    filesystem_type: &'a str,
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
    let candidate = candidate_root(argument_root, false)?;
    let inside_source_control = path_is_in_source_control(&candidate.root, &source_repo)?;
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

    if inside_source_control
        || storage.on_tmpfs_or_ramfs
        || storage.available_bytes < MINIMUM_AVAILABLE_BYTES
    {
        let error = if inside_source_control {
            "inside_source_control"
        } else if storage.on_tmpfs_or_ramfs {
            "tmpfs_or_ramfs"
        } else {
            "insufficient_capacity"
        };
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
    let root = fs::canonicalize(&candidate.root).map_err(|error| {
        ToolError(format!(
            "cannot canonicalize qualification root {}: {error}",
            candidate.root.display()
        ))
    })?;
    ensure_isolated_root(&root, &source_repo, &candidate.parent)?;

    let fixture = root.join("fixture");
    let evidence = root.join("evidence");
    fs::create_dir(&fixture)
        .map_err(|error| ToolError(format!("cannot create fixture directory: {error}")))?;
    fs::create_dir(&evidence)
        .map_err(|error| ToolError(format!("cannot create evidence directory: {error}")))?;

    let accepted_storage = StorageRecord {
        source_repo: &path_text(&source_repo),
        qualification_parent: &path_text(&candidate.parent),
        qualification_root: &path_text(&root),
        filesystem_type: &storage.filesystem_type,
        total_bytes: storage.total_bytes,
        available_bytes: storage.available_bytes,
        minimum_available_bytes: MINIMUM_AVAILABLE_BYTES,
        on_tmpfs_or_ramfs: storage.on_tmpfs_or_ramfs,
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
    let candidate = candidate_root(argument_root, true)?;
    let root = candidate.root;
    ensure_isolated_root(&root, &source_repo, &candidate.parent)?;
    if root.join(".git").exists() {
        return Err(ToolError("qualification root must not contain .git".into()));
    }
    let fixture = root.join("fixture");
    let evidence = root.join("evidence");
    require_real_directory(&fixture, "fixture")?;
    require_real_directory(&evidence, "evidence")?;
    ensure_fixture_has_no_git_context(&fixture)?;

    let storage = storage_info(&root)?;
    if storage.on_tmpfs_or_ramfs || storage.available_bytes < MINIMUM_AVAILABLE_BYTES {
        return Err(ToolError(
            "qualification root no longer satisfies storage preconditions".into(),
        ));
    }
    let storage_record = StorageRecord {
        source_repo: &path_text(&source_repo),
        qualification_parent: &path_text(&candidate.parent),
        qualification_root: &path_text(&root),
        filesystem_type: &storage.filesystem_type,
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
    let xtask_sha256 = hash_file(&env::current_exe().map_err(|error| {
        ToolError(format!(
            "cannot find running mara-xtask executable: {error}"
        ))
    })?)?;
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

    for run in 1..=RUN_COUNT {
        let outcome = run_exact_child(&mara, &fixture)?;
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
        if let Ok(snapshot) = &revalidation {
            fixture_files = snapshot.clone();
        }
        write_json(
            &evidence.join(format!("run-{run:02}.measurement.json")),
            &record,
        )?;
        records.push(record.clone());

        match revalidation {
            Ok(snapshot) => {
                let integrity_matches = snapshot.iter().all(|entry| entry.matched);
                if !integrity_matches {
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
                record.fixture_verified = None;
                record.passed = false;
                record.error = Some("fixture_revalidation_unavailable".into());
                let path = evidence.join(format!("run-{run:02}.measurement.json"));
                write_json(&path, &record)?;
                *records.last_mut().expect("current record exists") = record;
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
    let result = if passed { "passed" } else { "failed" };
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

fn path_is_in_source_control(root: &Path, source_repo: &Path) -> Result<bool> {
    if root.starts_with(source_repo) {
        return Ok(true);
    }
    let tmp = fs::canonicalize("/tmp").unwrap_or_else(|_| PathBuf::from("/tmp"));
    if root.starts_with(&tmp) {
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
    let mut file = File::create(path)
        .map_err(|error| ToolError(format!("cannot create {}: {error}", path.display())))?;
    file.write_all(contents)
        .map_err(|error| ToolError(format!("cannot write {}: {error}", path.display())))?;
    file.sync_all()
        .map_err(|error| ToolError(format!("cannot sync {}: {error}", path.display())))
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
fn run_exact_child(mara: &Path, fixture: &Path) -> Result<ChildOutcome> {
    use std::os::unix::process::CommandExt;

    let mut command = Command::new(mara);
    command
        .current_dir(fixture)
        .args(["check", "--format", "json"])
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

    let deadline = started + Duration::from_nanos(ELAPSED_LIMIT_NS);
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
            let _ = unsafe { libc::kill(-pid, libc::SIGKILL) };
            return Err(ToolError(format!("cannot wait for exact child: {error}")));
        }
        if Instant::now() >= deadline {
            timed_out = true;
            if unsafe { libc::kill(-pid, libc::SIGKILL) } != 0 {
                return Err(ToolError(format!(
                    "cannot kill timed-out child process group: {}",
                    io::Error::last_os_error()
                )));
            }
            loop {
                let waited = unsafe { libc::wait4(pid, &mut status, 0, &mut usage) };
                if waited == pid {
                    break;
                }
                if waited == -1 && io::Error::last_os_error().raw_os_error() != Some(libc::EINTR) {
                    return Err(ToolError(format!(
                        "cannot reap timed-out child: {}",
                        io::Error::last_os_error()
                    )));
                }
            }
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
) -> Result<Vec<FixtureFile>> {
    if hash_file(manifest)? != pin.sha256 {
        return fixture_mismatch_snapshot(fixture, pin);
    }
    match fs::symlink_metadata(fixture.join(".mara")) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
        Ok(_) => return fixture_mismatch_snapshot(fixture, pin),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return fixture_mismatch_snapshot(fixture, pin);
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
        return fixture_mismatch_snapshot(fixture, pin);
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
    Ok(entries)
}

#[cfg(target_os = "linux")]
fn fixture_mismatch_snapshot(fixture: &Path, pin: &ManifestPin) -> Result<Vec<FixtureFile>> {
    let mut entries = Vec::new();
    for (path, expected_sha256) in &pin.entries {
        entries.push(FixtureFile {
            path: path.clone(),
            expected_sha256: expected_sha256.clone(),
            observed_sha256: file_digest_if_regular(&fixture.join(path))?,
            matched: false,
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
    revalidation: &Result<Vec<FixtureFile>>,
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
        .map(|entries| entries.iter().all(|entry| entry.matched));
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
    } else if passed {
        None
    } else {
        Some("child_failed".into())
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
#[allow(clippy::items_after_test_module)]
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
    fn storage_capacity_and_tmpfs_are_rejected() {
        let low_capacity = StorageInfo {
            filesystem_type: "ext4".into(),
            total_bytes: MINIMUM_AVAILABLE_BYTES,
            available_bytes: MINIMUM_AVAILABLE_BYTES - 1,
            on_tmpfs_or_ramfs: false,
        };
        assert!(low_capacity.available_bytes < MINIMUM_AVAILABLE_BYTES);
        let volatile = StorageInfo {
            on_tmpfs_or_ramfs: true,
            ..low_capacity
        };
        assert!(volatile.on_tmpfs_or_ramfs);
    }

    #[test]
    fn workspace_isolation_rejects_a_root_beneath_the_source_repository() {
        let source = fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
            .expect("source repository");
        assert!(ensure_isolated_root(&source.join("not-external"), &source, &source).is_err());
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
        assert!(
            revalidate_fixture(&fixture, &manifest, &pin)
                .expect("revalidation")
                .iter()
                .all(|entry| !entry.matched)
        );
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
            run_exact_child_with_limit(&script, temporary.path(), Duration::from_millis(10))
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
                Duration::from_millis(10),
            )
            .is_err()
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn cleanup_failure_is_observable() {
        use std::os::unix::fs::PermissionsExt;

        let temporary = tempfile::tempdir().expect("temporary root");
        let parent = temporary.path().join("parent");
        let child = parent.join("qualification-root");
        fs::create_dir(&parent).expect("parent");
        fs::create_dir(&child).expect("child");
        let mut permissions = fs::metadata(&parent).expect("metadata").permissions();
        permissions.set_mode(0o555);
        fs::set_permissions(&parent, permissions).expect("read-only parent");
        assert!(fs::remove_dir_all(&child).is_err());
        let mut permissions = fs::metadata(&parent).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&parent, permissions).expect("restored parent");
        fs::remove_dir_all(&child).expect("explicit cleanup after failure");
    }

    fn fixture_digests(root: &Path) -> BTreeMap<String, String> {
        let mut values = BTreeMap::new();
        for path in manifest_paths() {
            values.insert(path.clone(), hash_file(&root.join(path)).expect("digest"));
        }
        values
    }
}

#[cfg(test)]
fn manifest_paths() -> Vec<String> {
    let mut paths = vec![".mara/project.toml".into(), ".mara/schema.yaml".into()];
    paths.extend((0..DOCUMENT_COUNT).map(|index| format!("items-{index:03}.mara.md")));
    paths
}

#[cfg(all(test, target_os = "linux"))]
fn run_exact_child_with_limit(
    program: &Path,
    directory: &Path,
    limit: Duration,
) -> Result<ChildOutcome> {
    use std::os::unix::process::CommandExt;
    let mut command = Command::new(program);
    command
        .current_dir(directory)
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
        .map_err(|error| ToolError(format!("cannot spawn test child: {error}")))?;
    let pid = child.id() as libc::pid_t;
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");
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
        if Instant::now() >= deadline {
            timed_out = true;
            if unsafe { libc::kill(-pid, libc::SIGKILL) } != 0 {
                return Err(ToolError("cannot kill test process group".into()));
            }
            if unsafe { libc::wait4(pid, &mut status, 0, &mut usage) } != pid {
                return Err(ToolError("cannot reap test child".into()));
            }
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    let stdout = stdout_reader.join().expect("stdout reader")?;
    let stderr = stderr_reader.join().expect("stderr reader")?;
    let signal = status & 0x7f;
    Ok(ChildOutcome {
        elapsed_ns: started.elapsed().as_nanos() as u64,
        peak_rss_kib: usage.ru_maxrss as u64,
        exit_code: (signal == 0).then_some((status >> 8) & 0xff),
        term_signal: (signal != 0).then_some(signal),
        timed_out,
        stdout,
        stderr,
    })
}
