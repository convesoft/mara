use std::{
    collections::BTreeMap,
    env,
    ffi::OsString,
    io::{self, Read, Write},
    path::PathBuf,
    process::ExitCode,
};

use clap::{Args, Parser, Subcommand, ValueEnum, error::ErrorKind};
use mara::{
    EntryRange, FieldValue, InitialRelation, ItemCollectionResult, ItemCreateParams,
    ItemFilterParams, ItemGetParams, ItemGetResult, ItemMoveParams, ItemRelatedParams,
    ItemSearchParams, ItemSummary, ItemUpdateParams, OperationContext, ProjectInitializationResult,
    ProjectMidBackfillResult, RelatedItem, RelationDirection, RelationMutationResult,
    RelationParams, RelationSummary, SchemaGetResult, SchemaKind, SchemaListResult,
    SchemaValidationResult, Template, ValidationResult, ValidationScope, ValidationTargetKind,
    project_initialize,
};
use serde::Serialize;

mod mcp;

#[derive(Debug, Parser)]
#[command(name = "mara", version, about = "Structured project knowledge")]
struct Cli {
    #[arg(
        long,
        global = true,
        value_name = "PATH",
        help = "Use this project root (absolute or relative to the working directory) instead of ancestor discovery; selects the init target or binds mcp"
    )]
    project: Option<PathBuf>,

    /// Select operation output as human-readable text or JSON (does not affect MCP).
    #[arg(long, global = true, value_enum, default_value_t)]
    format: OutputFormat,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Initialize, validate, or recover a Mara project.
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    /// Inspect and validate the effective project schema.
    Schema {
        #[command(subcommand)]
        command: SchemaCommand,
    },
    /// Author, retrieve, and validate project knowledge items.
    Item {
        #[command(subcommand)]
        command: ItemCommand,
    },
    /// Add or remove authored typed relations between items.
    Relation {
        #[command(subcommand)]
        command: RelationCommand,
    },
    /// Start a stdio MCP server, optionally bound with --project.
    Mcp,
}

#[derive(Debug, Subcommand)]
enum ProjectCommand {
    /// Initialize a Mara project without overwriting existing content.
    Init {
        /// Destination directory; defaults to the working directory. Cannot combine with --project.
        path: Option<PathBuf>,

        /// Initial schema: minimal includes common flavours and relations; empty declares none.
        #[arg(long, value_enum, default_value_t)]
        template: CliTemplate,
    },
    /// Validate the whole project, optionally selecting which diagnostics are shown.
    Validate {
        #[arg(
            long = "path",
            value_name = "PATH",
            help = "Show diagnostics for an exact document or directory subtree (project-relative, repeatable); project/schema errors always appear; validity and exit status still cover the whole project"
        )]
        paths: Vec<PathBuf>,
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
    /// Generate missing MIDs on legacy items after validation; preserve existing MIDs.
    Backfill,
}

#[derive(Debug, Subcommand)]
enum ItemCommand {
    /// Rename a human ID and its internal references, preserving the MID.
    Rename {
        /// Exact human ID or canonical MID (uppercase 26-character ULID, no prefix).
        reference: String,
        /// New unique human ID with the flavour's prefix; the old ID is not kept as an alias.
        new_id: String,
    },
    /// Delete one item only when no surviving relations or mentions refer to it.
    Delete {
        /// Exact human ID or canonical MID (uppercase 26-character ULID, no prefix).
        reference: String,
    },
    /// Partially update an item's title, custom fields, or body; supply at least one change.
    Update {
        /// Exact human ID or canonical MID (uppercase 26-character ULID, no prefix).
        reference: String,
        /// Replace the nonempty title; omission leaves it unchanged.
        #[arg(long)]
        title: Option<String>,
        /// Replace all values of a custom KEY=VALUE field; repeat for schema-repeatable keys.
        /// Excludes title, MID, and typed relations; use relation add/remove for edges.
        #[arg(long = "field", value_parser = parse_field)]
        fields: Vec<CliField>,
        /// Remove all values of an optional custom field (repeatable); cannot also set that key.
        #[arg(long = "clear-field")]
        clear_fields: Vec<String>,
        #[arg(
            long,
            help = "Replace body text; - reads stdin, an empty string clears an optional body, omission leaves it unchanged"
        )]
        body: Option<String>,
    },
    /// Move an item without changing its identity, content, or relations.
    Move {
        /// Exact human ID or canonical MID (uppercase 26-character ULID, no prefix).
        reference: String,
        /// Destination project-relative *.mara.md file; parent must exist and discovery must include it.
        file: PathBuf,
        /// Insert before this one-based line in the original destination; omission appends.
        #[arg(long)]
        line: Option<usize>,
    },
    /// Create an item with a generated MID and optional initial relations, or a body scaffold.
    Create {
        /// Schema-declared flavour; inspect with schema list flavour.
        flavour: String,
        /// New unique human ID with the flavour's prefix (for example REQ-EXAMPLE); not a MID.
        id: String,
        /// Destination project-relative *.mara.md file; parent must exist and discovery must include it.
        file: PathBuf,

        /// Nonempty single-line item title.
        #[arg(long)]
        title: String,

        #[arg(long = "field", value_parser = parse_field, help = "Schema-declared custom KEY=VALUE field; repeat only for repeatable keys. Excludes title, MID, and typed relations; use --relation or relation add for edges")]
        fields: Vec<CliField>,

        #[arg(long = "relation", value_name = "NAME=TARGET", value_parser = parse_initial_relation,
            help = "Initial outgoing relation, created atomically with the item (repeatable). TARGET is an exact human ID or canonical MID (uppercase 26-character ULID)")]
        relations: Vec<InitialRelation>,

        /// Body text, or - to read stdin; omission creates a scaffold when a body is required.
        #[arg(long)]
        body: Option<String>,

        /// Insert before this one-based line; omission appends, line_count + 1 means end of file.
        #[arg(long)]
        line: Option<usize>,
    },
    /// Read bounded consecutive portions of an item's body, metadata, and direct relations.
    Get {
        /// Exact human ID or canonical MID (uppercase 26-character ULID, no prefix).
        id: String,
        #[arg(
            long,
            help = "Maximum combined relation entries per page (1-100, default 20)"
        )]
        limit: Option<usize>,
        #[arg(
            long,
            help = "Opaque next_cursor from the previous page; keep id/limit unchanged until has_more is false; omit to start or restart after source/schema changes"
        )]
        cursor: Option<String>,
    },
    /// List compact item summaries in document-path and source order, with exact filters.
    List {
        #[command(flatten)]
        filters: ItemFilterArgs,
    },
    /// Rank word matches by relevance with typo tolerance; ID/MID words and filters stay exact.
    Search {
        /// Words to match across ID, title, body, and metadata; all words must match. Use "" for all items.
        query: String,

        #[command(flatten)]
        filters: ItemFilterArgs,

        #[arg(
            long = "id",
            help = "Select exact human IDs or canonical MIDs (uppercase 26-character ULIDs); repeat for OR, intersected with other filters"
        )]
        ids: Vec<String>,

        #[arg(
            long,
            help = "Include up to three bounded, partial source excerpts per match; use item get for complete content"
        )]
        excerpts: bool,
    },
    /// List direct incoming and outgoing relation entries; retrieve neighbour bodies with item get.
    Related {
        /// Exact human ID or canonical MID (uppercase 26-character ULID, no prefix).
        id: String,

        /// Select edge direction relative to this item; omission includes both.
        #[arg(long, value_enum)]
        direction: Option<CliRelationDirection>,

        /// Select exact relation names (repeatable, OR); intersects the neighbour flavour filter.
        #[arg(long)]
        relation: Vec<String>,

        /// Select exact neighbour flavours (repeatable, OR); omission includes all.
        #[arg(long)]
        flavour: Vec<String>,

        #[arg(long, help = "Page size: 1 through 100 (default 20)")]
        limit: Option<usize>,

        #[arg(
            long,
            help = "Opaque next_cursor from the previous page; keep item/options unchanged; omit to start or restart after source/schema changes"
        )]
        cursor: Option<String>,
    },
    /// Report all discoverable validation diagnostics applicable to one item.
    Validate {
        /// Exact human ID or canonical MID (uppercase 26-character ULID, no prefix).
        id: String,
    },
}

#[derive(Debug, Subcommand)]
enum RelationCommand {
    /// Add a schema-valid outgoing relation; rejects an existing edge.
    Add {
        /// Source item's exact human ID or canonical MID (uppercase 26-character ULID).
        source: String,
        /// Schema-declared relation name; inspect with schema list relation.
        relation: String,
        /// Target item's exact human ID or canonical MID (uppercase 26-character ULID).
        target: String,
    },
    /// Remove an authored outgoing relation; rejects a missing edge.
    Remove {
        /// Source item's exact human ID or canonical MID (uppercase 26-character ULID).
        source: String,
        /// Schema-declared relation name; inspect with schema list relation.
        relation: String,
        /// Target item's exact human ID or canonical MID (uppercase 26-character ULID).
        target: String,
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
    /// Select exact flavours (repeatable, OR); distinct filter categories combine with AND.
    #[arg(long)]
    flavour: Vec<String>,

    /// Exact metadata KEY=VALUE filter; OR within one key, AND across different keys (repeatable).
    #[arg(long = "field", value_parser = parse_field)]
    fields: Vec<CliField>,

    /// Select items with these authored outgoing relation names (repeatable, OR).
    #[arg(long)]
    relation: Vec<String>,

    #[arg(
        long,
        help = "Select an exact document or directory subtree (project-relative, repeatable OR), e.g. packages/query/docs/; no globs, absolute paths, or ..; omit for the whole project"
    )]
    path: Vec<PathBuf>,

    #[arg(long, help = "Page size: 1 through 100 (default 20)")]
    limit: Option<usize>,

    #[arg(
        long,
        help = "Opaque next_cursor from the previous page; keep query/options unchanged; omit to start or restart after source/schema changes"
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

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliRelationDirection {
    Incoming,
    Outgoing,
}

impl From<CliRelationDirection> for RelationDirection {
    fn from(value: CliRelationDirection) -> Self {
        match value {
            CliRelationDirection::Incoming => Self::Incoming,
            CliRelationDirection::Outgoing => Self::Outgoing,
        }
    }
}

#[derive(Debug, Subcommand)]
enum SchemaCommand {
    /// Get the complete effective schema, or one declaration by kind and name.
    Get {
        /// Declaration kind; supply with NAME, or omit both for the complete schema.
        #[arg(value_enum, requires = "name")]
        kind: Option<CliSchemaKind>,

        /// Exact declaration name; requires KIND.
        #[arg(requires = "kind")]
        name: Option<String>,
    },
    /// List declaration names and descriptions for one schema kind.
    List {
        /// Kind of schema declarations to list.
        #[arg(value_enum)]
        kind: CliSchemaKind,
    },
    /// Validate the project's configured schema without validating item content.
    Validate,
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
}

impl From<CliTemplate> for Template {
    fn from(value: CliTemplate) -> Self {
        match value {
            CliTemplate::Minimal => Self::Minimal,
            CliTemplate::Empty => Self::Empty,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
enum OutputFormat {
    #[default]
    Human,
    Json,
}

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
        Command::Mcp => {
            mcp::run(project)?;
            Ok(true)
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
            let result = project_initialize(target, template.into())?;
            emit(format, &result, print_project_initialization)?;
            Ok(true)
        }
        Command::Project {
            command: ProjectCommand::Validate { paths },
        } => {
            let result = operations(project)?.project_validate(&paths)?;
            emit_validation(format, &result)
        }
        Command::Project {
            command:
                ProjectCommand::Mid {
                    command: ProjectMidCommand::Backfill,
                },
        } => {
            let result = operations(project)?.project_mid_backfill()?;
            emit(format, &result, print_project_mid_backfill)?;
            Ok(true)
        }
        Command::Project {
            command:
                ProjectCommand::Transaction {
                    command: ProjectTransactionCommand::Rollback,
                },
        } => {
            let result = operations(project)?.project_transaction_rollback()?;
            emit(format, &result, |result| {
                if result.restored.is_empty() {
                    println!("no pending transaction");
                }
                for path in &result.restored {
                    println!("rolled back {}", path.display());
                }
                Ok(())
            })?;
            Ok(true)
        }
        Command::Item {
            command: ItemCommand::Rename { reference, new_id },
        } => {
            let result = operations(project)?.item_rename(&reference, &new_id)?;
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
            Ok(true)
        }
        Command::Item {
            command: ItemCommand::Delete { reference },
        } => {
            let result = operations(project)?.item_delete(&reference)?;
            emit(format, &result, |result| {
                println!(
                    "deleted item '{}' with MID {} from {}",
                    result.id,
                    result.mid,
                    result.path.display()
                );
                Ok(())
            })?;
            Ok(true)
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
            let result = operations(project)?.item_update(ItemUpdateParams {
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
                        warning.path.as_ref().expect("item warning path").display(),
                        warning.line.expect("item warning line"),
                        warning.message
                    );
                }
                Ok(())
            })?;
            Ok(true)
        }
        Command::Item {
            command:
                ItemCommand::Move {
                    reference,
                    file,
                    line,
                },
        } => {
            let result = operations(project)?.item_move(ItemMoveParams {
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
            Ok(true)
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
            let result = operations(project)?.item_create(ItemCreateParams {
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
            Ok(true)
        }
        Command::Item {
            command: ItemCommand::Validate { id },
        } => {
            let result = operations(project)?.item_validate(&id)?;
            emit_validation(format, &result)
        }
        Command::Item {
            command: ItemCommand::Get { id, limit, cursor },
        } => {
            let result = operations(project)?.item_get(ItemGetParams { id, limit, cursor })?;
            emit(format, &result, |item| {
                print_resolved_item(item);
                Ok(())
            })?;
            Ok(true)
        }
        Command::Item {
            command: ItemCommand::List { filters },
        } => {
            let result = operations(project)?.item_list(filters.into_params())?;
            emit(format, &result, print_item_collection)?;
            Ok(true)
        }
        Command::Item {
            command:
                ItemCommand::Search {
                    query,
                    filters,
                    ids,
                    excerpts,
                },
        } => {
            let filters = filters.into_params();
            let result = operations(project)?.item_search(ItemSearchParams {
                query,
                flavours: filters.flavours,
                fields: filters.fields,
                relations: filters.relations,
                paths: filters.paths,
                limit: filters.limit,
                cursor: filters.cursor,
                ids,
                excerpts,
            })?;
            emit(format, &result, print_item_collection)?;
            Ok(true)
        }
        Command::Item {
            command:
                ItemCommand::Related {
                    id,
                    direction,
                    relation,
                    flavour,
                    limit,
                    cursor,
                },
        } => {
            let result = operations(project)?.item_related(ItemRelatedParams {
                id,
                direction: direction.map(Into::into),
                relations: relation,
                flavours: flavour,
                limit,
                cursor,
            })?;
            emit(format, &result, |result| {
                print_related_items(&result.items);
                print_page_continuation(result.has_more, result.next_cursor.as_deref());
                Ok(())
            })?;
            Ok(true)
        }
        Command::Relation {
            command:
                RelationCommand::Add {
                    source,
                    relation,
                    target,
                },
        } => {
            let result = operations(project)?.relation_add(RelationParams {
                source,
                relation,
                target,
            })?;
            emit(format, &result, print_relation_mutation)?;
            Ok(true)
        }
        Command::Relation {
            command:
                RelationCommand::Remove {
                    source,
                    relation,
                    target,
                },
        } => {
            let result = operations(project)?.relation_remove(RelationParams {
                source,
                relation,
                target,
            })?;
            emit(format, &result, print_relation_mutation)?;
            Ok(true)
        }
        Command::Schema {
            command: SchemaCommand::Get { kind, name },
        } => {
            let result = operations(project)?.schema_get(kind.map(Into::into), name)?;
            emit(format, &result, print_schema_get)?;
            Ok(true)
        }
        Command::Schema {
            command: SchemaCommand::List { kind },
        } => {
            let result = operations(project)?.schema_list(kind.into())?;
            emit(format, &result, print_schema_list)?;
            Ok(true)
        }
        Command::Schema {
            command: SchemaCommand::Validate,
        } => {
            let result = operations(project)?.schema_validate()?;
            emit(format, &result, print_schema_validation)?;
            Ok(true)
        }
    }
}

fn operations(selected: Option<PathBuf>) -> Result<OperationContext, String> {
    OperationContext::from_environment(selected)
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

fn print_resolved_item(item: &ItemGetResult) {
    print_item_heading(&item.summary);
    let source = &item.source;
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
    println!("body");
    print!("{}", item.body);
    if !item.body.ends_with('\n') {
        println!();
    }
    println!(
        "body_range\tstart_byte={}\tend_byte={}\ttotal_bytes={}\tpartial={}",
        item.body_range.start_byte,
        item.body_range.end_byte,
        item.body_range.total_bytes,
        item.body_range.partial
    );
    println!("relations");
    for relation in &item.outgoing_relations {
        print_relation_summary(RelationDirection::Outgoing, relation);
    }
    for relation in &item.incoming_relations {
        print_relation_summary(RelationDirection::Incoming, relation);
    }
    print_entry_range("outgoing_relations_range", &item.outgoing_relations_range);
    print_entry_range("incoming_relations_range", &item.incoming_relations_range);
    print_page_continuation(item.has_more, item.next_cursor.as_deref());
}

fn print_entry_range(label: &str, range: &EntryRange) {
    println!(
        "{label}\tstart_index={}\tend_index={}\ttotal={}\tpartial={}",
        range.start_index, range.end_index, range.total, range.partial
    );
}

fn print_item_collection(result: &ItemCollectionResult) -> Result<(), String> {
    for item in &result.items {
        print_item_summary(item);
        if let Some(excerpts) = item.excerpts() {
            for excerpt in excerpts {
                println!(
                    "excerpt\tpartial=true\t{}:{}-{}\tbytes={}-{}\t{}",
                    item.path().display(),
                    excerpt.start_line,
                    excerpt.end_line,
                    excerpt.start_byte,
                    excerpt.end_byte,
                    serde_json::to_string(&excerpt.text).map_err(|error| error.to_string())?
                );
            }
        }
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

fn print_item_heading(item: &ItemSummary) {
    let title = if item.title_truncated() {
        format!("{} [title truncated]", item.title())
    } else {
        item.title().to_owned()
    };
    if let Some(mid) = item.mid() {
        println!("{}\t{}\t{}\t{}", item.id(), mid, item.flavour(), title);
    } else {
        println!("{}\t{}\t{}", item.id(), item.flavour(), title);
    }
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

fn print_relation_summary(direction: RelationDirection, relation: &RelationSummary) {
    print_related_line(direction, relation.relation(), relation.item());
}

fn print_related_items(items: &[RelatedItem]) {
    for item in items {
        print_related_line(item.direction(), item.relation(), item.item());
    }
}

fn print_related_line(direction: RelationDirection, relation: &str, item: &ItemSummary) {
    let title = if item.title_truncated() {
        format!("{} [title truncated]", item.title())
    } else {
        item.title().to_owned()
    };
    if let Some(mid) = item.mid() {
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}:{}",
            direction.as_str(),
            relation,
            item.id(),
            mid,
            item.flavour(),
            title,
            item.path().display(),
            item.line()
        );
    } else {
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}:{}",
            direction.as_str(),
            relation,
            item.id(),
            item.flavour(),
            title,
            item.path().display(),
            item.line()
        );
    }
}

fn print_relation_mutation(result: &RelationMutationResult) -> Result<(), String> {
    println!(
        "{} relation '{}' from '{}' to '{}' in {}",
        result.action.past_tense(),
        result.relation,
        result.source,
        result.target,
        result.path.display()
    );
    Ok(())
}

fn emit_validation(format: OutputFormat, result: &ValidationResult) -> Result<bool, String> {
    emit(format, result, print_validation)?;
    Ok(result.valid)
}

fn print_validation(result: &ValidationResult) -> Result<(), String> {
    if result.valid {
        match result.target.kind {
            ValidationTargetKind::Project => {
                println!("valid project at {}", result.project.display());
            }
            ValidationTargetKind::Item => {
                println!(
                    "valid item '{}'",
                    result
                        .target
                        .id
                        .as_deref()
                        .expect("item validation has an item ID")
                );
            }
        }
        return Ok(());
    }
    for diagnostic in &result.diagnostics {
        match diagnostic.scope {
            ValidationScope::Project => eprintln!(
                "error: invalid Mara project at {}: {}",
                diagnostic
                    .path
                    .as_deref()
                    .expect("project diagnostic has a path")
                    .display(),
                diagnostic.message
            ),
            ValidationScope::Schema => eprintln!(
                "error: invalid Mara schema at {}: {}",
                diagnostic
                    .path
                    .as_deref()
                    .expect("schema diagnostic has a path")
                    .display(),
                diagnostic.message
            ),
            ValidationScope::Item => eprintln!("error: {}", diagnostic.message),
            ValidationScope::Document => eprintln!(
                "{}:{}: error: {}",
                diagnostic
                    .path
                    .as_deref()
                    .expect("document diagnostic has a path")
                    .display(),
                diagnostic.line.expect("document diagnostic has a line"),
                diagnostic.message
            ),
        }
    }
    let omitted = result
        .selection
        .as_ref()
        .map_or(0, |selection| selection.omitted_diagnostics);
    if omitted > 0 {
        eprintln!(
            "error: {omitted} diagnostic{} outside the selection omitted",
            if omitted == 1 { "" } else { "s" }
        );
    }
    let count = result.diagnostics.len() + omitted;
    eprintln!(
        "error: validation failed with {count} diagnostic{}",
        if count == 1 { "" } else { "s" }
    );
    Ok(())
}

fn print_schema_get(result: &SchemaGetResult) -> Result<(), String> {
    match result {
        SchemaGetResult::Schema { schema } => print_yaml(schema),
        SchemaGetResult::Flavour { name, definition } => print_named_yaml(name, definition),
        SchemaGetResult::Relation { name, definition } => print_named_yaml(name, definition),
    }
}

fn print_schema_list(result: &SchemaListResult) -> Result<(), String> {
    for declaration in &result.declarations {
        println!("{}\t{}", declaration.name, declaration.description);
    }
    Ok(())
}

fn print_schema_validation(result: &SchemaValidationResult) -> Result<(), String> {
    println!(
        "valid schema at {} ({} flavours, {} relations)",
        result.path.display(),
        result.flavours,
        result.relations
    );
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
