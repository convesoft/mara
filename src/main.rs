use clap::{Args, Parser, Subcommand, ValueEnum, error::ErrorKind};
use mara::{
    EntryRange, GetParams, GetResult, RelatedConnection, RelatedParams, RelationDirection,
    SearchParams,
};
use mara::{FieldValue, ItemCollectionResult, ItemFilterParams, ItemSummary};
use mara::{InitialRelation, ItemCreateParams, ItemUpdateParams};
use mara::{
    OperationContext, ProjectInitializationResult, SchemaGetResult, SchemaKind, SchemaListResult,
    Template, ValidationOptions, ValidationResult, ValidationTargetKind, project_initialize,
};
use mara::{ProjectMidBackfillResult, RelationParams};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    env,
    ffi::OsString,
    io::{self, Read, Write},
    path::PathBuf,
    process::ExitCode,
};
mod mcp;

#[derive(Debug, Parser)]
#[command(
    name = "mara",
    version,
    about = "Structured project knowledge",
    after_help = "Start with project init, then inspect schema guidance before authoring. Search discovers items and narrative; get reads content and related explores direct connections. Item and relation commands edit canonical source. Schema, project and item validation report conformance; trace matrix inspects selected coverage. Use --project to select a project and --format json for structured CLI results. The mcp command serves these operations over stdio."
)]
struct Cli {
    /// Use an absolute or working-directory-relative project root instead of ancestor discovery; selects the init target or binds MCP.
    #[arg(long, global = true, value_name = "PATH")]
    project: Option<PathBuf>,
    /// Select human-readable or JSON output (does not affect MCP).
    #[arg(long, global = true, value_enum, default_value_t)]
    format: OutputFormat,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Search items and narrative with one source excerpt; exact matches rank first, then ID/title/heading weights. Item filters exclude narrative; ID/MID words and filters stay exact.
    #[command(
        after_help = "Discovery JSON format_version: 2 returns results: [{node, excerpt}]. Pass node.reference to get or related. Excerpts support selection; use get for complete content. Structural handles identify a document snapshot; search again after that document changes. Search has no node-kind filter."
    )]
    Search {
        /// Unicode case-insensitive words to match across items, section headings, and ordinary Markdown blocks; every distinct word must match. An empty string or punctuation-only text matches all search units within the filters.
        query: String,

        #[command(flatten)]
        filters: ItemFilterArgs,

        #[arg(
            long = "id",
            help = "Select exact human IDs or canonical MIDs (uppercase 26-character ULIDs); repeat for OR, intersected with other filters. Omission adds no restriction"
        )]
        ids: Vec<String>,
    },

    /// Read an item, section, Markdown block, document, or code endpoint in bounded consecutive portions.
    #[command(
        after_help = "Discovery JSON format_version: 2 returns node, content, content_range, metadata, and metadata_range. Items return their parsed body; sections and documents include contained Markdown source. Non-items have empty metadata. Reconstruct content and ordered metadata fragments using byte/index ranges until has_more is false. Get has no limit option and does not enumerate neighbours; use related. Search again if a structural handle is stale. Code symbols use the configured language name and exact SCIP descriptor; copy references from related. No name/position fallback. Preserve literal backticks and single-quote code references in the shell. Project format 3 configures external SCIP commands in [[code.languages]]; corpus reads invoke them automatically when files match the required extensions list. No matches skips that indexer. Tree-sitter grammar/query assets are optional for comment attachment and declaration content."
    )]
    Get {
        /// Exact item ID/MID, a code:<path>[::<language>::<descriptor>] reference, or a discovery handle.
        reference: String,
        #[arg(
            long,
            help = "Opaque next_cursor from the previous page; keep reference unchanged until has_more is false. Omit to start or restart after source/schema changes. Empty strings are invalid"
        )]
        cursor: Option<String>,
    },

    /// Explore direct schema relations, code backlinks, mentions, and containment with source evidence.
    #[command(
        after_help = "Discovery JSON format_version: 2 returns node and connections: schema edges have relation, label, direction, neighbour, edge and occurrence_count; builtin connections retain source. Inspect authored locations with relation get. Internal neighbours have a reference for get/related; external neighbours have only kind and address and are terminal. JSON represents containment as contains with direction; human output displays its incoming view as contained_by. Use --relation builtin:contains --direction incoming for the parent, then outgoing on that parent for its children. Search again if a structural handle is stale."
    )]
    Related {
        /// Exact item ID/MID, a code:<path>[::<language>::<descriptor>] reference, or a discovery handle.
        reference: String,

        /// Select edge direction relative to this node; omission includes incoming, outgoing and symmetric, outgoing first. Incoming/outgoing exclude symmetric edges.
        #[arg(long, value_enum)]
        direction: Option<CliRelationDirection>,

        /// Select relation names (schema:name or builtin:name; shorthand only when unambiguous) (repeatable, OR); intersects the neighbour flavour filter. Omission includes all.
        #[arg(long)]
        relation: Vec<String>,

        /// Select exact neighbour flavours (repeatable, OR); nonempty selects item neighbours only, omission includes all.
        #[arg(long)]
        flavour: Vec<String>,

        #[arg(
            long,
            help = "Maximum relation entries per page: 1 through 100 (default 20), not unique neighbours; the byte budget may return fewer"
        )]
        limit: Option<usize>,

        #[arg(
            long,
            help = "Opaque next_cursor from the previous page; keep reference/options unchanged until has_more is false; omit to start or restart after source/schema changes. Empty strings are invalid"
        )]
        cursor: Option<String>,
    },

    /// Inspect canonical relationships and their authored occurrences.
    Relation {
        #[command(subcommand)]
        command: RelationCommand,
    },
    /// Create, inspect, validate and edit structured items.
    Item {
        #[command(subcommand)]
        command: ItemCommand,
    },
    /// Initialize or validate a project, backfill MIDs, or recover a pending mutation.
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    /// Inspect or validate the selected project's schema declarations.
    Schema {
        #[command(subcommand)]
        command: SchemaCommand,
    },
    /// Render read-only trace views from an explicit item selection.
    Trace {
        #[command(subcommand)]
        command: TraceCommand,
    },
    /// Serve the available operations over stdio MCP; resolve projects per call.
    Mcp,
}

#[derive(Debug, Subcommand)]
enum TraceCommand {
    /// Generate a bounded, read-only coverage matrix using enabled rule IRIs or a request-local YAML check.
    #[command(
        after_help = "Select roots with --all or one or more --id, --flavour, --field and --path filters. Use --rule for enabled root rules, or --check-file with --shape for a request-only check; do not mix them. Example for a check containing hasValue: {parameter: subject_revision}: mara trace matrix --id REQ-A --check-file rules/revision.yaml --shape urn:mara:rule:revision_evidence --param subject_revision=abc123. Parameters are exact text values and may also occur in in lists. Default output is Markdown; --format json returns trace format 1. Read result states, checks, edges, summaries and evaluation_complete; follow --cursor with unchanged inputs until has_more is false. The view does not change project policy or source files."
    )]
    Matrix {
        /// Exact human ID or MID for a root item; repeat for OR and intersect with other root filters.
        #[arg(long = "id")]
        ids: Vec<String>,
        /// Exact schema flavour for root items; repeat for OR and intersect with other root filters.
        #[arg(long = "flavour")]
        flavours: Vec<String>,
        /// Exact custom KEY=VALUE root filter without trimming; an empty value matches an empty value. Excludes title/MID and typed relations.
        #[arg(long = "field", value_name = "KEY=VALUE")]
        fields: Vec<String>,
        /// Project-relative document or directory subtree for root items; no globs, absolute paths, .., empty paths, . or ./.
        #[arg(long = "path")]
        paths: Vec<PathBuf>,
        /// Explicitly select all root items; cannot be combined with filters.
        #[arg(long)]
        all: bool,
        /// Expanded IRI of an enabled root rule; repeat for OR. Cannot combine with a request check.
        #[arg(long = "rule")]
        rules: Vec<String>,
        /// Project-relative YAML file for a request-local check; repeat to supply reusable shapes.
        #[arg(long = "check-file")]
        check_files: Vec<PathBuf>,
        /// Expanded IRI of a named targetless node shape from the request check files.
        #[arg(long)]
        shape: Option<String>,
        /// Named text literal for a request check, as NAME=VALUE; repeat for different names.
        #[arg(long = "param", value_name = "NAME=VALUE")]
        parameters: Vec<String>,
        /// Maximum records per page, 1 through 100 (default 20); the byte budget may return fewer.
        #[arg(long)]
        limit: Option<usize>,
        /// Opaque next_cursor; keep options unchanged until has_more is false; restart after source/schema changes; empty strings are invalid.
        #[arg(long)]
        cursor: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
enum ProjectCommand {
    /// Validate the whole project; paths select reporting only.
    #[command(
        after_help = "Configured [[code.languages]] entries require source extensions. SCIP commands run automatically from the project root during corpus loading, including validation, only when unignored source files match. Empty languages are skipped without invoking their command. Install indexers separately and configure trusted commands. Command or index failures make evaluation incomplete; a valid file link does not prove intended symbol attachment. Inspect code backlinks with related and relation get."
    )]
    Validate {
        /// Exact document or directory subtree relative to the project; repeat for OR. No globs, absolute paths, .., empty paths, . or ./; omit --path for the whole project. Selects reported diagnostics only; validity covers the whole project.
        #[arg(long = "path")]
        paths: Vec<PathBuf>,
        /// Maximum diagnostics per page, 1 through 100 (default 20); the byte budget may return fewer.
        #[arg(long)]
        limit: Option<usize>,
        /// Opaque next_cursor; repeat unchanged options until has_more is false. Restart after source/schema changes; empty strings are invalid.
        #[arg(long)]
        cursor: Option<String>,
    },
    /// Initialize the current or named directory without overwriting existing files.
    Init {
        /// Destination; omit to use --project, or the current directory only when --project is also omitted. Cannot combine with --project.
        path: Option<PathBuf>,
        /// Bundled schema: minimal (default), empty, or engineering with policy/check files.
        #[arg(long, value_enum, default_value_t)]
        template: CliTemplate,
    },
    /// Recover a pending multi-file mutation.
    Transaction {
        #[command(subcommand)]
        command: ProjectTransactionCommand,
    },
    /// Manage durable machine identities (MIDs).
    Mid {
        #[command(subcommand)]
        command: ProjectMidCommand,
    },
}

#[derive(Debug, Subcommand)]
enum ProjectTransactionCommand {
    /// Restore original files from a pending mutation journal; stop other writers first.
    ///
    /// Preserves later manual edits by refusing conflicting files. No journal is a no-op.
    Rollback,
}

#[derive(Debug, Subcommand)]
enum ProjectMidCommand {
    /// Generate MIDs for items that lack them after validation; preserve existing MIDs.
    Backfill,
}

#[derive(Debug, Subcommand)]
enum SchemaCommand {
    /// Validate schema declarations and configured YAML rule definitions without evaluating items.
    Validate {
        /// Maximum diagnostics per page, 1 through 100 (default 20); the byte budget may return fewer.
        #[arg(long)]
        limit: Option<usize>,
        /// Opaque next_cursor; repeat unchanged options until has_more is false. Restart after source/schema changes; empty strings are invalid.
        #[arg(long)]
        cursor: Option<String>,
    },
    /// Get the full effective schema or one named flavour/relation declaration, including authoring guidance.
    Get {
        /// Supply KIND and NAME together, or omit both for the complete schema.
        #[arg(value_enum, requires = "name")]
        kind: Option<CliSchemaKind>,
        /// Exact declaration name; relation inverse aliases are resolved to their canonical declaration.
        #[arg(requires = "kind")]
        name: Option<String>,
    },
    /// List canonical declaration names and descriptions; follow with get for full guidance.
    List {
        /// Kind of declarations to list: flavour or relation.
        #[arg(value_enum)]
        kind: CliSchemaKind,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliSchemaKind {
    Flavour,
    Relation,
}

impl From<CliSchemaKind> for SchemaKind {
    fn from(value: CliSchemaKind) -> Self {
        match value {
            CliSchemaKind::Flavour => Self::Flavour,
            CliSchemaKind::Relation => Self::Relation,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
enum CliTemplate {
    #[default]
    Minimal,
    Empty,
    Engineering,
}

impl From<CliTemplate> for Template {
    fn from(value: CliTemplate) -> Self {
        match value {
            CliTemplate::Minimal => Self::Minimal,
            CliTemplate::Empty => Self::Empty,
            CliTemplate::Engineering => Self::Engineering,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
enum OutputFormat {
    #[default]
    Human,
    Json,
}

// @mara implements REQ-SURFACE-PARITY
fn main() -> ExitCode {
    let arguments = env::args_os().collect::<Vec<_>>();
    let requested_format = requested_output_format(&arguments);
    let cli = match Cli::try_parse_from(arguments) {
        Ok(cli) => cli,
        Err(error) => return report_parse_error(error, requested_format),
    };
    let format = cli.format;
    match run(cli) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            if matches!(format, OutputFormat::Json) {
                if let Err(render_error) = write_json(&serde_json::json!({
                    "error": { "message": error }
                })) {
                    eprintln!("error: {render_error}");
                }
            } else {
                eprintln!("error: {error}");
            }
            ExitCode::FAILURE
        }
    }
}

fn requested_output_format(arguments: &[OsString]) -> OutputFormat {
    let mut format = OutputFormat::Human;
    let mut arguments = arguments.iter().skip(1);

    while let Some(argument) = arguments.next() {
        let Some(argument) = argument.to_str() else {
            continue;
        };
        let value = if argument == "--format" {
            arguments.next().and_then(|argument| argument.to_str())
        } else {
            argument.strip_prefix("--format=")
        };

        match value {
            Some("json") => format = OutputFormat::Json,
            Some("human") => format = OutputFormat::Human,
            _ => {}
        }
    }

    format
}

fn report_parse_error(error: clap::Error, format: OutputFormat) -> ExitCode {
    let exit_code = ExitCode::from(u8::try_from(error.exit_code()).unwrap_or(1));
    if matches!(
        error.kind(),
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
    ) {
        if let Err(render_error) = error.print() {
            eprintln!("error: could not render command help: {render_error}");
        }
    } else if matches!(format, OutputFormat::Json) {
        if let Err(render_error) = write_json(&serde_json::json!({
            "error": { "message": error.to_string() }
        })) {
            eprintln!("error: {render_error}");
        }
    } else if let Err(render_error) = error.print() {
        eprintln!("error: could not render command error: {render_error}");
    }
    exit_code
}

fn run(cli: Cli) -> Result<bool, String> {
    let Cli {
        project,
        format,
        command,
    } = cli;
    match command {
        Command::Trace {
            command:
                TraceCommand::Matrix {
                    ids,
                    flavours,
                    fields,
                    paths,
                    all,
                    rules,
                    check_files,
                    shape,
                    parameters,
                    limit,
                    cursor,
                },
        } => {
            let fields = fields
                .into_iter()
                .map(|field| {
                    let (key, value) = field.split_once('=').ok_or_else(|| {
                        mara::ValidationError::invalid_argument("--field must use KEY=VALUE")
                    })?;
                    Ok(mara::TraceField {
                        key: key.into(),
                        value: value.into(),
                    })
                })
                .collect::<Result<Vec<_>, mara::ValidationError>>();
            let fields = match fields {
                Ok(fields) => fields,
                Err(error) => return emit_trace_error(format, error),
            };
            let mut bindings = std::collections::BTreeMap::new();
            for parameter in parameters {
                let Some((name, value)) = parameter.split_once('=') else {
                    return emit_trace_error(
                        format,
                        mara::ValidationError::invalid_argument("--param must use NAME=VALUE"),
                    );
                };
                if bindings
                    .insert(name.to_owned(), serde_json::json!(value))
                    .is_some()
                {
                    return emit_trace_error(
                        format,
                        mara::ValidationError::invalid_argument(format!(
                            "duplicate check parameter '{name}'"
                        )),
                    );
                }
            }
            if check_files.is_empty() && shape.is_none() && !bindings.is_empty() {
                return emit_trace_error(
                    format,
                    mara::ValidationError::invalid_argument(
                        "--param requires --check-file and --shape",
                    ),
                );
            }
            let check = if check_files.is_empty() && shape.is_none() && bindings.is_empty() {
                None
            } else {
                Some(mara::TraceCheck {
                    files: check_files,
                    shape: shape.unwrap_or_default(),
                    parameters: bindings,
                })
            };
            let params = mara::TraceMatrixParams {
                selection: mara::TraceSelection {
                    ids,
                    flavours,
                    fields,
                    paths,
                    all,
                },
                rules,
                check,
                limit,
                cursor,
                render: matches!(format, OutputFormat::Human).then(|| "markdown".into()),
            };
            return match OperationContext::from_environment(project)?.trace_matrix(&params) {
                Ok(result) => {
                    if matches!(format, OutputFormat::Json) {
                        write_json(&result)?
                    } else {
                        print!("{}", result.markdown.as_deref().unwrap_or(""))
                    }
                    Ok(result.evaluation_complete)
                }
                Err(error) => {
                    if matches!(format, OutputFormat::Json) {
                        write_json(&error.envelope())?
                    } else {
                        eprintln!("error: {error}")
                    }
                    Ok(false)
                }
            };
        }

        Command::Relation {
            command:
                RelationCommand::Get {
                    source,
                    relation,
                    target,
                    limit,
                    cursor,
                },
        } => {
            return emit_relation(
                format,
                OperationContext::from_environment(project)?.relation_get(
                    RelationParams {
                        source,
                        relation,
                        target,
                    },
                    limit,
                    cursor,
                ),
                |result| {
                    println!(
                        "{} {} {}: {} occurrences",
                        result.edge.source.id(),
                        result.edge.relation,
                        display_relation_target(&result.edge.target),
                        result.occurrence_count
                    );
                    for entry in &result.occurrences {
                        println!(
                            "{}:{} {} {} selector={}",
                            entry.source.path().display(),
                            entry.source.start_line(),
                            entry.relation,
                            entry.target,
                            entry.reference
                        );
                    }
                    print_page_continuation(result.has_more, result.next_cursor.as_deref());
                    Ok(())
                },
            );
        }
        Command::Relation {
            command:
                RelationCommand::Add {
                    source,
                    relation,
                    target,
                },
        } => {
            return emit_relation(
                format,
                OperationContext::from_environment(project)?.relation_add(RelationParams {
                    source,
                    relation,
                    target,
                }),
                print_relation_mutation,
            );
        }
        Command::Relation {
            command:
                RelationCommand::Remove {
                    source,
                    relation,
                    target,
                    occurrence,
                },
        } => {
            return emit_relation(
                format,
                OperationContext::from_environment(project)?.relation_remove_occurrence(
                    RelationParams {
                        source,
                        relation,
                        target,
                    },
                    occurrence,
                ),
                print_relation_mutation,
            );
        }
        Command::Item {
            command:
                ItemCommand::Update {
                    reference,
                    title,
                    fields,
                    clear_fields,
                    body,
                },
        } => {
            let body = read_body(body)?;
            let result =
                OperationContext::from_environment(project)?.item_update(ItemUpdateParams {
                    reference,
                    title,
                    fields: fields.into_iter().map(Into::into).collect(),
                    clear_fields,
                    body,
                })?;
            emit(format, &result, |result| {
                println!(
                    "updated item '{}' with MID {} at {}",
                    result.id,
                    result.mid,
                    result.path.display()
                );
                println!("changed fields: {}", result.changed_fields.join(", "));
                for warning in &result.warnings {
                    eprintln!(
                        "warning: {}:{}: {}",
                        warning.path.display(),
                        warning.line,
                        warning.message
                    );
                }
                Ok(())
            })?;
        }
        Command::Item {
            command: ItemCommand::Delete { reference },
        } => {
            let result = OperationContext::from_environment(project)?.item_delete(&reference)?;
            emit(format, &result, |result| {
                println!(
                    "deleted item '{}' with MID {} from {}",
                    result.id,
                    result.mid,
                    result.path.display()
                );
                Ok(())
            })?;
        }
        Command::Item {
            command:
                ItemCommand::Move {
                    reference,
                    file,
                    line,
                },
        } => {
            let result =
                OperationContext::from_environment(project)?.item_move(mara::ItemMoveParams {
                    reference,
                    file,
                    line,
                })?;
            emit(format, &result, |result| {
                println!(
                    "moved item '{}' with MID {} from {}:{} to {}:{}",
                    result.id,
                    result.mid,
                    result.old_location.path.display(),
                    result.old_location.line,
                    result.new_location.path.display(),
                    result.new_location.line
                );
                Ok(())
            })?;
        }
        Command::Item {
            command: ItemCommand::Rename { reference, new_id },
        } => {
            let result =
                OperationContext::from_environment(project)?.item_rename(&reference, &new_id)?;
            emit(format, &result, |result| {
                println!(
                    "renamed item '{}' to '{}' with MID {}",
                    result.old_id, result.new_id, result.mid
                );
                for path in &result.paths {
                    println!("updated {}", path.display());
                }
                Ok(())
            })?;
        }
        Command::Related {
            reference,
            direction,
            relation,
            flavour,
            limit,
            cursor,
        } => {
            let result = OperationContext::from_environment(project)?.related(RelatedParams {
                reference,
                direction: direction.map(Into::into),
                relations: relation,
                flavours: flavour,
                limit,
                cursor,
            })?;
            emit(format, &result, |result| {
                print_related_connections(&result.connections);
                print_page_continuation(result.has_more, result.next_cursor.as_deref());
                Ok(())
            })?;
        }
        Command::Get { reference, cursor } => {
            let result = OperationContext::from_environment(project)?
                .get(GetParams { reference, cursor })?;
            emit(format, &result, |item| {
                print_get(item);
                Ok(())
            })?;
        }
        Command::Search {
            query,
            filters,
            ids,
        } => {
            let filters = filters.into_params();
            let result = OperationContext::from_environment(project)?.search(SearchParams {
                query,
                flavours: filters.flavours,
                fields: filters.fields,
                relations: filters.relations,
                paths: filters.paths,
                limit: filters.limit,
                cursor: filters.cursor,
                ids,
            })?;
            emit(format, &result, |page| {
                for hit in &page.results {
                    let node = &hit.node;
                    let kind = node.block_kind.map_or_else(
                        || format!("{:?}", node.kind),
                        |kind| format!("Block({kind:?})"),
                    );
                    println!(
                        "{}\t{}\t{}:{}\t{}{}",
                        node.id.as_deref().unwrap_or(&node.reference),
                        kind,
                        node.source.path().display(),
                        node.source.start_line(),
                        node.title.as_deref().unwrap_or_default(),
                        if node.title_truncated {
                            " [title truncated]"
                        } else {
                            ""
                        }
                    );
                    println!(
                        "excerpt\tpartial={}\t{}:{}\t{}",
                        hit.excerpt.partial,
                        node.source.path().display(),
                        hit.excerpt.start_line,
                        hit.excerpt.text
                    );
                }
                print_page_continuation(page.has_more, page.next_cursor.as_deref());
                Ok(())
            })?;
        }

        Command::Item {
            command:
                ItemCommand::Create {
                    flavour,
                    id,
                    file,
                    title,
                    fields,
                    relations,
                    body,
                    line,
                },
        } => {
            let body = read_body(body)?;
            let result =
                OperationContext::from_environment(project)?.item_create(ItemCreateParams {
                    flavour,
                    id,
                    file,
                    title,
                    fields: fields.into_iter().map(Into::into).collect(),
                    relations,
                    body,
                    line,
                })?;
            emit(format, &result, |result| {
                println!(
                    "created item '{}' with MID {} at {}:{}",
                    result.id,
                    result.mid,
                    result.path.display(),
                    result.line
                );
                println!("complete: {}", result.complete);
                for missing in &result.missing {
                    println!("missing: {missing}");
                }
                Ok(())
            })?;
        }
        Command::Item {
            command: ItemCommand::List { filters },
        } => {
            let result =
                OperationContext::from_environment(project)?.item_list(filters.into_params())?;
            emit(format, &result, print_item_collection)?;
        }
        Command::Mcp => mcp::run(project)?,
        Command::Project {
            command:
                ProjectCommand::Mid {
                    command: ProjectMidCommand::Backfill,
                },
        } => {
            let result = OperationContext::from_environment(project)?.project_mid_backfill()?;
            emit(format, &result, print_project_mid_backfill)?;
        }
        Command::Project {
            command:
                ProjectCommand::Transaction {
                    command: ProjectTransactionCommand::Rollback,
                },
        } => {
            let result =
                OperationContext::from_environment(project)?.project_transaction_rollback()?;
            emit(format, &result, |result| {
                if result.restored.is_empty() {
                    println!("no pending transaction");
                }
                for path in &result.restored {
                    println!("rolled back {}", path.display());
                }
                Ok(())
            })?;
        }
        Command::Project {
            command: ProjectCommand::Init { path, template },
        } => {
            let target = match (project, path) {
                (Some(project), None) => project,
                (None, Some(path)) => path,
                (None, None) => PathBuf::from("."),
                (Some(_), Some(_)) => {
                    return Err("--project and project init PATH cannot be used together".into());
                }
            };
            emit(
                format,
                &project_initialize(target, template.into())?,
                print_project_initialization,
            )?;
        }
        Command::Schema {
            command: SchemaCommand::Validate { limit, cursor },
        } => {
            let result = OperationContext::from_environment(project)?
                .schema_validate_with_options(&ValidationOptions { limit, cursor });
            return emit_validation(format, result);
        }
        Command::Project {
            command:
                ProjectCommand::Validate {
                    paths,
                    limit,
                    cursor,
                },
        } => {
            let result = OperationContext::from_environment(project)?
                .project_validate_with_options(&paths, &ValidationOptions { limit, cursor });
            return emit_validation(format, result);
        }
        Command::Item {
            command: ItemCommand::Validate { id, limit, cursor },
        } => {
            let result = OperationContext::from_environment(project)?
                .item_validate_with_options(&id, &ValidationOptions { limit, cursor });
            return emit_validation(format, result);
        }
        Command::Schema {
            command: SchemaCommand::Get { kind, name },
        } => {
            let result = OperationContext::from_environment(project)?
                .schema_get(kind.map(Into::into), name)?;
            emit(format, &result, print_schema_get)?;
        }
        Command::Schema {
            command: SchemaCommand::List { kind },
        } => {
            let result = OperationContext::from_environment(project)?.schema_list(kind.into())?;
            emit(format, &result, print_schema_list)?;
        }
    }
    Ok(true)
}

fn emit_trace_error(format: OutputFormat, error: mara::ValidationError) -> Result<bool, String> {
    if matches!(format, OutputFormat::Json) {
        write_json(&error.envelope())?
    } else {
        eprintln!("error: {error}")
    }
    Ok(false)
}

fn emit<T: Serialize>(
    format: OutputFormat,
    value: &T,
    human: impl FnOnce(&T) -> Result<(), String>,
) -> Result<(), String> {
    match format {
        OutputFormat::Human => human(value),
        OutputFormat::Json => write_json(value),
    }
}

fn write_json(value: &impl Serialize) -> Result<(), String> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    serde_json::to_writer(&mut stdout, value)
        .map_err(|error| format!("could not render JSON output: {error}"))?;
    writeln!(stdout).map_err(|error| format!("could not write JSON output: {error}"))
}

fn print_project_initialization(result: &ProjectInitializationResult) -> Result<(), String> {
    println!(
        "initialized Mara project at {}",
        result.project.root.display()
    );
    for path in &result.created {
        println!("created {}", path.display());
    }
    Ok(())
}

fn print_schema_get(result: &SchemaGetResult) -> Result<(), String> {
    match result {
        SchemaGetResult::Schema { schema } => print_yaml(schema),
        SchemaGetResult::Flavour { name, definition } => print_named_yaml(name, definition),
        SchemaGetResult::Relation {
            name, definition, ..
        } => print_named_yaml(name, definition),
    }
}

fn print_schema_list(result: &SchemaListResult) -> Result<(), String> {
    for declaration in &result.declarations {
        println!(
            "{}\t{}{}{}",
            declaration.name,
            declaration.description,
            declaration
                .inverse
                .as_ref()
                .map(|alias| format!("\tinverse={alias}"))
                .unwrap_or_default(),
            if declaration.symmetric == Some(true) {
                "\tsymmetric"
            } else {
                ""
            }
        );
    }
    Ok(())
}

fn print_named_yaml<T: Serialize>(name: &str, definition: &T) -> Result<(), String> {
    let declarations = BTreeMap::from([(name, definition)]);
    print_yaml(&declarations)
}

fn print_yaml(value: &impl Serialize) -> Result<(), String> {
    let source = serde_saphyr::to_string(value)
        .map_err(|error| format!("could not render schema: {error}"))?;
    print!("{source}");
    Ok(())
}

fn emit_validation(
    format: OutputFormat,
    result: Result<ValidationResult, mara::ValidationError>,
) -> Result<bool, String> {
    match result {
        Ok(result) => {
            emit(format, &result, print_validation)?;
            Ok(result.valid)
        }
        Err(error) => {
            if matches!(format, OutputFormat::Json) {
                write_json(&error.envelope())?;
            } else {
                eprintln!("error: {error}");
            }
            Ok(false)
        }
    }
}

fn print_validation(result: &ValidationResult) -> Result<(), String> {
    for diagnostic in &result.diagnostics {
        let location = diagnostic
            .location
            .path
            .as_ref()
            .map(|p| {
                let line = diagnostic
                    .location
                    .line
                    .map(|n| format!(":{n}"))
                    .unwrap_or_default();
                format!("{}{line}: ", p.display())
            })
            .unwrap_or_default();
        let code = serde_json::to_value(diagnostic.code).expect("diagnostic code");
        eprintln!(
            "{location}{}: {} [{}]",
            diagnostic.severity,
            diagnostic.message,
            code.as_str().unwrap()
        );
    }
    let omitted = result
        .selection
        .as_ref()
        .map_or(0, |s| s.omitted_diagnostics);
    if omitted > 0 {
        eprintln!("{omitted} diagnostics outside the selection omitted");
    }
    if result.has_more {
        eprintln!(
            "more diagnostics; repeat with --cursor {} and unchanged options",
            result.next_cursor.as_deref().unwrap()
        );
    }
    if result.valid {
        match result.target.kind {
            ValidationTargetKind::Project => {
                println!("valid project at {}", result.project.display())
            }
            ValidationTargetKind::Item => {
                println!("valid item '{}'", result.target.id.as_deref().unwrap())
            }
            ValidationTargetKind::Schema => println!(
                "valid schema at {} ({} flavours, {} relations)",
                result.path.as_ref().unwrap().display(),
                result.flavours.flatten().unwrap(),
                result.relations.flatten().unwrap()
            ),
        }
    } else {
        let count = result.summary.errors + result.summary.warnings;
        eprintln!(
            "error: validation failed with {count} diagnostic{}",
            if count == 1 { "" } else { "s" }
        );
    }
    if !result.evaluation_complete {
        eprintln!("evaluation incomplete; diagnostic counts are lower bounds");
    }
    Ok(())
}

#[derive(Debug, Subcommand)]
enum ItemCommand {
    /// Validate one exact human ID or MID in full corpus context.
    Validate {
        /// Exact human ID or canonical MID (uppercase 26-character ULID without a prefix).
        id: String,
        /// Maximum diagnostics per page, 1 through 100 (default 20); the byte budget may return fewer.
        #[arg(long)]
        limit: Option<usize>,
        /// Opaque next_cursor; repeat unchanged options until has_more is false. Restart after source/schema changes; empty strings are invalid.
        #[arg(long)]
        cursor: Option<String>,
    },
    /// Rename a human ID and supported typed relations/wiki mentions in items and narrative across a valid project, preserving the MID. Markdown links are preserved, not rewritten.
    Rename {
        /// Exact human ID or canonical MID (uppercase 26-character ULID, no prefix).
        reference: String,
        /// New unique human ID with the flavour's prefix; the old ID is not kept as an alias. The current ID is a no-op.
        new_id: String,
    },

    /// Move an item within a valid project without changing its identity, content, or relations; keep the source document.
    #[command(
        after_help = "ID/MID references retain item identity. Relative Markdown links carried with the item, incoming links to its contained nodes, and shifted heading anchors must keep their destinations. Resolve reported link impacts before retrying; Markdown links are not automatically repaired."
    )]
    Move {
        /// Exact human ID or canonical MID (uppercase 26-character ULID, no prefix).
        reference: String,
        /// Destination project-relative *.mara.md file; parent must exist and discovery must include it. Creates the file if absent; no absolute paths or .. components.
        file: PathBuf,
        /// Insert before this one-based line in the original destination, including same-file moves; valid range is 1 through line_count + 1 (end of file). Omission appends. Insertion inside an item is rejected; an unchanged-location move may publish byte-identical source.
        #[arg(long)]
        line: Option<usize>,
    },

    /// Delete one item from a valid project only when no surviving typed relations, wiki mentions, or Markdown links refer to it or its contained nodes; keep the containing document.
    #[command(
        after_help = "Reject changes that break or retarget surviving internal links, including generated heading anchors affected elsewhere in the document. Resolve reported source locations before retrying; Markdown links are not automatically repaired."
    )]
    Delete {
        /// Exact human ID or canonical MID (uppercase 26-character ULID, no prefix).
        reference: String,
    },

    /// Partially update title, custom fields, or body while preserving identity.
    Update {
        /// Exact human ID or canonical MID (uppercase 26-character ULID, no prefix).
        reference: String,
        /// Replacement single-line title; surrounding whitespace is trimmed. Empty or whitespace-only titles and line breaks are rejected; omission leaves it unchanged.
        #[arg(long)]
        title: Option<String>,
        /// Replace all values of a custom KEY=VALUE field; repeat for schema-repeatable keys.
        /// KEY= keeps an empty value; use --clear-field KEY to remove the field.
        /// Values are schema-validated scalar text: surrounding whitespace is trimmed and line breaks are rejected. Omission leaves fields unchanged.
        /// Excludes title, MID, and typed relations; use relation add/remove for edges.
        #[arg(long = "field", value_parser = parse_field)]
        fields: Vec<CliField>,
        /// Remove all values of an optional custom field (repeatable); cannot also set that key. Excludes title/MID and typed relations. Omission clears nothing; an absent optional field is a no-op.
        #[arg(long = "clear-field")]
        clear_fields: Vec<String>,
        #[arg(
            long,
            help = "Replace body text; - reads stdin, an empty string clears an optional body, omission leaves it unchanged. Empty or whitespace-only replacements of required bodies are rejected"
        )]
        body: Option<String>,
    },

    /// Create an item with a generated MID and optional initial relations, or a body scaffold.
    #[command(
        after_help = "Choose a flavour from schema get flavour <NAME>: description explains purpose, use_when gives selection criteria, avoid_when gives exclusions, and distinguish_from compares other flavours. These are schema guidance, not item fields. Inspect relation endpoints before adding initial edges. Newly authored references must resolve; creation rejects changes that break or retarget surviving links, including shifted heading anchors."
    )]
    Create {
        /// Schema-declared flavour; discover names with schema list flavour, then read selection guidance with schema get flavour NAME.
        flavour: String,
        /// New unique human ID with the flavour's prefix (for example REQ-EXAMPLE); not a MID.
        id: String,
        /// Destination project-relative *.mara.md file; parent must exist and discovery must include it. Creates the file if absent; no absolute paths or .. components.
        file: PathBuf,

        /// Single-line item title; surrounding whitespace is trimmed. Empty or whitespace-only titles and line breaks are rejected.
        #[arg(long)]
        title: String,

        #[arg(long = "field", value_parser = parse_field, help = "Schema-declared custom KEY=VALUE field; repeat only for repeatable keys. Values are schema-validated scalar text: surrounding whitespace is trimmed and line breaks are rejected. KEY= supplies an empty value when schema-valid; supply all required fields. Excludes title, MID, and typed relations; use --relation or relation add for edges")]
        fields: Vec<CliField>,

        #[arg(long = "relation", value_name = "NAME=TARGET", value_parser = parse_initial_relation,
            help = "Initial schema-declared outgoing relation, created atomically with the item (repeatable). TARGET is an exact human ID, canonical MID, or external:HTTP(S) URL; the new ID may target itself. Duplicate edges are rejected; omission adds none. Later edits use relation add/remove")]
        relations: Vec<InitialRelation>,

        /// Body text, or - to read stdin; supports [[relation:ID]] and [[relation:MID]] assertions. An omitted, empty, or whitespace-only required body creates an incomplete scaffold.
        #[arg(long)]
        body: Option<String>,

        /// Insert before this one-based line; valid range is 1 through line_count + 1 (end of file). Omission appends. Insertion inside another item is rejected.
        #[arg(long)]
        line: Option<usize>,
    },

    /// List bounded item summaries in document-path and source order.
    List {
        #[command(flatten)]
        filters: ItemFilterArgs,
    },
}
#[derive(Debug, Clone)]
struct CliField {
    key: String,
    value: String,
}

impl From<CliField> for FieldValue {
    fn from(value: CliField) -> Self {
        Self {
            key: value.key,
            value: value.value,
        }
    }
}

#[derive(Debug, Args)]
struct ItemFilterArgs {
    /// Select exact flavours (repeatable, OR); distinct filter categories combine with AND. Omission selects all flavours.
    #[arg(long)]
    flavour: Vec<String>,

    /// Exact schema-declared custom-field KEY=VALUE filter; excludes title/MID and typed relations. Key and scalar text value match exactly, without trimming; KEY= matches an empty value. OR within one key, AND across keys and other filter categories (repeatable). Omission adds no restriction.
    #[arg(long = "field", value_parser = parse_field)]
    fields: Vec<CliField>,

    /// Select items with these exact authored outgoing schema relation names (repeatable, OR). Inverse aliases match their canonical declaration; search accepts schema:name for ambiguous built-in names. Omission adds no restriction.
    #[arg(long)]
    relation: Vec<String>,

    #[arg(
        long,
        help = "Select an exact document or directory subtree (project-relative, repeatable OR), e.g. packages/query/docs/; no glob expansion, absolute paths, .., empty paths, . or ./; interior dot components and repeated separators normalize; omit --path for the whole project"
    )]
    path: Vec<PathBuf>,

    #[arg(
        long,
        help = "Maximum entries per page: 1 through 100 (default 20); the byte budget may return fewer"
    )]
    limit: Option<usize>,

    #[arg(
        long,
        help = "Opaque next_cursor from the previous page; keep all other inputs unchanged until has_more is false; omit to start or restart after source/schema changes. Empty strings are invalid"
    )]
    cursor: Option<String>,
}

impl ItemFilterArgs {
    fn into_params(self) -> ItemFilterParams {
        ItemFilterParams {
            flavours: self.flavour,
            fields: self.fields.into_iter().map(Into::into).collect(),
            relations: self.relation,
            paths: self.path,
            limit: self.limit,
            cursor: self.cursor,
        }
    }
}

fn parse_field(value: &str) -> Result<CliField, String> {
    let (key, value) = value
        .split_once('=')
        .ok_or_else(|| "field must use KEY=VALUE".to_owned())?;
    if key.is_empty() {
        return Err("field key must not be empty".into());
    }
    Ok(CliField {
        key: key.to_owned(),
        value: value.to_owned(),
    })
}

fn print_item_collection(result: &ItemCollectionResult) -> Result<(), String> {
    for item in &result.items {
        print_item_summary(item);
    }
    print_page_continuation(result.has_more, result.next_cursor.as_deref());
    Ok(())
}

fn print_page_continuation(has_more: bool, next_cursor: Option<&str>) {
    print!("page\thas_more={has_more}");
    if let Some(cursor) = next_cursor {
        print!("\tnext_cursor={cursor}");
    }
    println!();
}

fn print_item_summary(item: &ItemSummary) {
    let title = if item.title_truncated() {
        format!("{} [title truncated]", item.title())
    } else {
        item.title().to_owned()
    };
    if let Some(mid) = item.mid() {
        println!(
            "{}\t{}\t{}\t{}\t{}:{}",
            item.id(),
            mid,
            item.flavour(),
            title,
            item.path().display(),
            item.line()
        );
    } else {
        println!(
            "{}\t{}\t{}\t{}:{}",
            item.id(),
            item.flavour(),
            title,
            item.path().display(),
            item.line()
        );
    }
}

fn print_get(item: &GetResult) {
    let node = &item.node;
    let kind = node.block_kind.map_or_else(
        || format!("{:?}", node.kind),
        |kind| format!("Block({kind:?})"),
    );
    println!(
        "{}\t{}\t{}{}",
        node.reference,
        kind,
        node.title.as_deref().unwrap_or(""),
        if node.title_truncated {
            " [title truncated]"
        } else {
            ""
        }
    );
    if let Some(id) = &node.id {
        println!("id\t{id}");
    }
    if let Some(flavour) = &node.flavour {
        println!("flavour\t{flavour}");
    }
    if let Some(level) = node.heading_level {
        println!("heading_level\t{level}");
    }
    if let Some(parent) = &node.context.parent {
        println!("parent\t{parent}");
    }
    if let Some(section) = &node.context.section {
        println!("section\t{section}");
    }
    let source = &node.source;
    println!(
        "source\t{}\tstart_byte={}\tend_byte={}\tstart_line={}\tend_line={}",
        source.path().display(),
        source.start_byte(),
        source.end_byte(),
        source.start_line(),
        source.end_line()
    );
    println!("metadata");
    for entry in &item.metadata {
        println!("{}\t{}", entry.key, entry.value);
        println!(
            "metadata_fragment\tindex={}\tstart_byte={}\tend_byte={}\ttotal_bytes={}\tpartial={}",
            entry.index,
            entry.range.start_byte,
            entry.range.end_byte,
            entry.range.total_bytes,
            entry.range.partial
        );
    }
    print_entry_range("metadata_range", &item.metadata_range);
    println!("content");
    print!("{}", item.content);
    if !item.content.ends_with('\n') {
        println!();
    }
    println!(
        "content_range\tstart_byte={}\tend_byte={}\ttotal_bytes={}\tpartial={}",
        item.content_range.start_byte,
        item.content_range.end_byte,
        item.content_range.total_bytes,
        item.content_range.partial
    );
    print_page_continuation(item.has_more, item.next_cursor.as_deref());
}

fn print_entry_range(label: &str, range: &EntryRange) {
    println!(
        "{label}\tstart_index={}\tend_index={}\ttotal={}\tpartial={}",
        range.start_index, range.end_index, range.total, range.partial
    );
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliRelationDirection {
    Incoming,
    Outgoing,
    Symmetric,
}

impl From<CliRelationDirection> for RelationDirection {
    fn from(value: CliRelationDirection) -> Self {
        match value {
            CliRelationDirection::Incoming => Self::Incoming,
            CliRelationDirection::Outgoing => Self::Outgoing,
            CliRelationDirection::Symmetric => Self::Symmetric,
        }
    }
}

fn print_related_connections(connections: &[RelatedConnection]) {
    for connection in connections {
        let node = match &connection.neighbour {
            mara::RelatedNeighbour::Internal(node) => node,
            mara::RelatedNeighbour::External { address, .. } => {
                println!(
                    "{} → external:{}\toccurrences={}",
                    connection.label.as_deref().unwrap_or(&connection.relation),
                    address,
                    connection.occurrence_count.unwrap_or_default()
                );
                continue;
            }
        };
        if let Some(edge) = &connection.edge {
            let label = connection.label.as_deref().unwrap_or(&edge.relation);
            let prefix =
                if connection.direction == RelationDirection::Incoming && label == edge.relation {
                    "incoming "
                } else {
                    ""
                };
            println!(
                "{prefix}{label} → {}\t{}{}\t{}:{}\toccurrences={}\treference={}",
                node.id.as_deref().unwrap_or(&node.reference),
                node.title.as_deref().unwrap_or_default(),
                if node.title_truncated {
                    " [title truncated]"
                } else {
                    ""
                },
                node.source.path().display(),
                node.source.start_line(),
                connection.occurrence_count.unwrap(),
                node.reference
            );
        } else {
            let relation = match (connection.relation.as_str(), connection.direction) {
                ("contains", RelationDirection::Incoming) => "contained_by",
                ("builtin:contains", RelationDirection::Incoming) => "builtin:contained_by",
                (name, _) => name,
            };
            let source = connection.source.as_ref().expect("builtin source");
            println!(
                "{}\t{}\t{}\t{}\t{}{}\t{}:{}\tevidence={}:{}-{}\treference={}",
                connection.direction.as_str(),
                relation,
                node.id.as_deref().unwrap_or(&node.reference),
                format!("{:?}", node.kind).to_lowercase(),
                node.title.as_deref().unwrap_or_default(),
                if node.title_truncated {
                    " [title truncated]"
                } else {
                    ""
                },
                node.source.path().display(),
                node.source.start_line(),
                source.path().display(),
                source.start_line(),
                source.end_line(),
                node.reference
            );
        }
    }
}

#[derive(Debug, Subcommand)]
enum RelationCommand {
    /// Inspect a semantic edge and its authored source occurrences.
    Get {
        /// Item ID/MID or code:<path>[::<language>::<descriptor>] expressing the relation.
        source: String,
        /// Canonical relation name or declared inverse alias.
        relation: String,
        /// Other endpoint: item ID/MID, code:<path>[::<language>::<descriptor>], or external:HTTP(S) URL.
        target: String,
        /// Maximum occurrences per page, 1 through 100; defaults to 20. The byte budget may return fewer.
        #[arg(long)]
        limit: Option<usize>,
        /// Continue with next_cursor and unchanged arguments until has_more is false; restart after source/schema changes; empty strings are invalid.
        #[arg(long)]
        cursor: Option<String>,
    },
    /// Add a schema-valid relation. Code links require an item source and declared inverse; code files are never edited.
    Add {
        /// Source item's exact human ID or canonical MID (uppercase 26-character ULID); code sources are read-only.
        source: String,
        /// Schema-declared relation name; inspect with schema list relation.
        relation: String,
        /// Target item's exact human ID, canonical MID, external:HTTP(S) URL, or code:<path>[::<language>::<descriptor>] with an inverse alias.
        target: String,
    },
    /// Remove authored item assertions. For code links, code comment markers remain and may keep the edge present.
    Remove {
        /// Source item's exact human ID or canonical MID (uppercase 26-character ULID); code sources are read-only.
        source: String,
        /// Schema-declared relation name; inspect with schema list relation.
        relation: String,
        /// Target item's exact human ID, canonical MID, external:HTTP(S) URL, or code:<path>[::<language>::<descriptor>] with an inverse alias.
        target: String,
        /// Remove only this snapshot-bound occurrence from relation get.
        #[arg(long)]
        occurrence: Option<String>,
    },
}

fn emit_relation<T: Serialize>(
    format: OutputFormat,
    result: Result<T, mara::RelationError>,
    human: impl FnOnce(&T) -> Result<(), String>,
) -> Result<bool, String> {
    match result {
        Ok(result) => {
            emit(format, &result, human)?;
            Ok(true)
        }
        Err(error) => {
            if matches!(format, OutputFormat::Json) {
                write_json(&error)?;
            } else {
                eprintln!("error: {error}");
            }
            Ok(false)
        }
    }
}

fn display_relation_target(target: &mara::RelationEndpoint) -> String {
    match target {
        mara::RelationEndpoint::Item { id, .. } => id.clone(),
        mara::RelationEndpoint::External { address } => format!("external:{address}"),
        mara::RelationEndpoint::Code { reference } => reference.clone(),
    }
}

fn print_project_mid_backfill(result: &ProjectMidBackfillResult) -> Result<(), String> {
    if result.changed.is_empty() {
        println!("no missing MIDs in project at {}", result.project.display());
        return Ok(());
    }
    println!(
        "backfilled {} MID{} in project at {}",
        result.changed.len(),
        if result.changed.len() == 1 { "" } else { "s" },
        result.project.display()
    );
    for entry in &result.changed {
        println!(
            "{}\t{}\t{}:{}",
            entry.id,
            entry.mid,
            entry.path.display(),
            entry.line
        );
    }
    Ok(())
}

fn parse_initial_relation(value: &str) -> Result<InitialRelation, String> {
    let (relation, target) = value
        .split_once('=')
        .ok_or_else(|| "relation must use NAME=TARGET".to_owned())?;
    if relation.is_empty() || target.is_empty() {
        return Err("relation name and target must not be empty".into());
    }
    Ok(InitialRelation {
        relation: relation.to_owned(),
        target: target.to_owned(),
    })
}

fn read_body(body: Option<String>) -> Result<Option<String>, String> {
    match body.as_deref() {
        Some("-") => {
            let mut body = String::new();
            io::stdin()
                .read_to_string(&mut body)
                .map_err(|error| format!("could not read item body from stdin: {error}"))?;
            Ok(Some(body))
        }
        _ => Ok(body),
    }
}

fn print_relation_mutation(result: &mara::RelationMutationResult) -> Result<(), String> {
    println!(
        "{} relation '{}' from '{}' to '{}': {} changed, {} remaining",
        result.action.past_tense(),
        result.edge.relation,
        result.edge.source.id(),
        display_relation_target(&result.edge.target),
        result.changed_occurrences,
        result.remaining_occurrences
    );
    Ok(())
}
