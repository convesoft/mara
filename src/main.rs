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
    FieldValue, ItemCollectionResult, ItemCreateParams, ItemFilterParams, ItemMoveParams,
    ItemRelatedParams, ItemSummary, ItemUpdateParams, OperationContext,
    ProjectInitializationResult, ProjectMidBackfillResult, RelatedItem, RelationDirection,
    RelationMutationResult, RelationParams, RelationSummary, ResolvedItem, SchemaGetResult,
    SchemaKind, SchemaListResult, SchemaValidationResult, Template, ValidationResult,
    ValidationScope, ValidationTargetKind, project_initialize,
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
        help = "Use this Mara project root instead of discovery or an init PATH"
    )]
    project: Option<PathBuf>,

    #[arg(long, global = true, value_enum, default_value_t)]
    format: OutputFormat,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    Schema {
        #[command(subcommand)]
        command: SchemaCommand,
    },
    Item {
        #[command(subcommand)]
        command: ItemCommand,
    },
    Relation {
        #[command(subcommand)]
        command: RelationCommand,
    },
    /// Start a stdio MCP server, optionally bound with --project.
    Mcp,
}

#[derive(Debug, Subcommand)]
enum ProjectCommand {
    Init {
        path: Option<PathBuf>,

        #[arg(long, value_enum, default_value_t)]
        template: CliTemplate,
    },
    Validate,
    Transaction {
        #[command(subcommand)]
        command: ProjectTransactionCommand,
    },
    Mid {
        #[command(subcommand)]
        command: ProjectMidCommand,
    },
}

#[derive(Debug, Subcommand)]
enum ProjectTransactionCommand {
    /// Restore original files from a pending mutation journal.
    Rollback,
}

#[derive(Debug, Subcommand)]
enum ProjectMidCommand {
    Backfill,
}

#[derive(Debug, Subcommand)]
enum ItemCommand {
    /// Rename a human ID and its internal references, preserving the MID.
    Rename {
        reference: String,
        new_id: String,
    },
    /// Delete one item only when no surviving relations or mentions refer to it.
    Delete {
        reference: String,
    },
    /// Partially update an item's title, custom fields, or body.
    Update {
        reference: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long = "field", value_parser = parse_field)]
        fields: Vec<CliField>,
        #[arg(long = "clear-field")]
        clear_fields: Vec<String>,
        #[arg(long, help = "Replace the body; use - to read standard input")]
        body: Option<String>,
    },
    Move {
        reference: String,
        file: PathBuf,
        #[arg(long)]
        line: Option<usize>,
    },
    Create {
        flavour: String,
        id: String,
        file: PathBuf,

        #[arg(long)]
        title: String,

        #[arg(long = "field", value_parser = parse_field)]
        fields: Vec<CliField>,

        #[arg(long)]
        body: Option<String>,

        #[arg(long)]
        line: Option<usize>,
    },
    Get {
        id: String,
    },
    List {
        #[command(flatten)]
        filters: ItemFilterArgs,
    },
    Search {
        query: String,

        #[command(flatten)]
        filters: ItemFilterArgs,
    },
    Related {
        id: String,

        #[arg(long, value_enum)]
        direction: Option<CliRelationDirection>,

        #[arg(long)]
        relation: Vec<String>,

        #[arg(long)]
        flavour: Vec<String>,
    },
    Validate {
        id: String,
    },
}

#[derive(Debug, Subcommand)]
enum RelationCommand {
    Add {
        source: String,
        relation: String,
        target: String,
    },
    Remove {
        source: String,
        relation: String,
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
    #[arg(long)]
    flavour: Vec<String>,

    #[arg(long = "field", value_parser = parse_field)]
    fields: Vec<CliField>,

    #[arg(long)]
    relation: Vec<String>,

    #[arg(long)]
    path: Vec<PathBuf>,

    #[arg(long)]
    limit: Option<usize>,
}

impl ItemFilterArgs {
    fn into_params(self) -> ItemFilterParams {
        ItemFilterParams {
            flavours: self.flavour,
            fields: self.fields.into_iter().map(Into::into).collect(),
            relations: self.relation,
            paths: self.path,
            limit: self.limit,
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
    Get {
        #[arg(value_enum, requires = "name")]
        kind: Option<CliSchemaKind>,

        #[arg(requires = "kind")]
        name: Option<String>,
    },
    List {
        #[arg(value_enum)]
        kind: CliSchemaKind,
    },
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
            command: ProjectCommand::Validate,
        } => {
            let result = operations(project)?.project_validate()?;
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
            command: ItemCommand::Get { id },
        } => {
            let result = operations(project)?.item_get(&id)?;
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
            command: ItemCommand::Search { query, filters },
        } => {
            let result = operations(project)?.item_search(&query, filters.into_params())?;
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
                },
        } => {
            let result = operations(project)?.item_related(ItemRelatedParams {
                id,
                direction: direction.map(Into::into),
                relations: relation,
                flavours: flavour,
            })?;
            emit(format, &result, |result| {
                print_related_items(&result.items);
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

fn print_resolved_item(item: &ResolvedItem) {
    print_item_heading(item.summary());
    let source = item.source();
    println!(
        "source\t{}\tstart_byte={}\tend_byte={}\tstart_line={}\tend_line={}",
        source.path().display(),
        source.start_byte(),
        source.end_byte(),
        source.start_line(),
        source.end_line()
    );
    println!("metadata");
    for entry in item.metadata() {
        println!("{}\t{}", entry.key(), entry.value());
    }
    println!("body");
    print!("{}", item.body());
    if !item.body().ends_with('\n') {
        println!();
    }
    println!("relations");
    for relation in item.outgoing_relations() {
        print_relation_summary(RelationDirection::Outgoing, relation);
    }
    for relation in item.incoming_relations() {
        print_relation_summary(RelationDirection::Incoming, relation);
    }
}

fn print_item_collection(result: &ItemCollectionResult) -> Result<(), String> {
    print_item_summaries(&result.items);
    Ok(())
}

fn print_item_summaries(items: &[ItemSummary]) {
    for item in items {
        print_item_summary(item);
    }
}

fn print_item_heading(item: &ItemSummary) {
    if let Some(mid) = item.mid() {
        println!(
            "{}\t{}\t{}\t{}",
            item.id(),
            mid,
            item.flavour(),
            item.title()
        );
    } else {
        println!("{}\t{}\t{}", item.id(), item.flavour(), item.title());
    }
}

fn print_item_summary(item: &ItemSummary) {
    if let Some(mid) = item.mid() {
        println!(
            "{}\t{}\t{}\t{}\t{}:{}",
            item.id(),
            mid,
            item.flavour(),
            item.title(),
            item.path().display(),
            item.line()
        );
    } else {
        println!(
            "{}\t{}\t{}\t{}:{}",
            item.id(),
            item.flavour(),
            item.title(),
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
    if let Some(mid) = item.mid() {
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}:{}",
            direction.as_str(),
            relation,
            item.id(),
            mid,
            item.flavour(),
            item.title(),
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
            item.title(),
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
    let count = result.diagnostics.len();
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
