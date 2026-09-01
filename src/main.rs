use std::{
    collections::BTreeMap,
    env,
    io::{self, Read},
    path::PathBuf,
    process::ExitCode,
};

use clap::{Args, Parser, Subcommand, ValueEnum};
use mara::{
    Diagnostic, FieldFilter, ItemCreationRequest, ItemFilters, ItemSummary, RelatedFilters,
    RelatedItem, RelationDirection, RelationSummary, ResolvedItem, Schema, Template, add_relation,
    create_item, get_item, initialize_project, list_items, load_corpus, load_corpus_for_validation,
    load_corpus_syntax_for_validation, load_schema, load_schema_for_validation, related_items,
    remove_relation, resolve_project, resolve_project_for_validation, search_items,
    validate_corpus, validate_corpus_independent,
};
use serde::Serialize;

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
}

#[derive(Debug, Subcommand)]
enum ProjectCommand {
    Init {
        path: Option<PathBuf>,

        #[arg(long, value_enum, default_value_t)]
        template: CliTemplate,
    },
    Validate,
}

#[derive(Debug, Subcommand)]
enum ItemCommand {
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
    fn into_domain(self) -> ItemFilters {
        ItemFilters::new(
            self.flavour,
            self.fields
                .into_iter()
                .map(|field| FieldFilter::new(field.key, field.value))
                .collect(),
            self.relation,
            self.path,
            self.limit,
        )
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
        kind: Option<SchemaKind>,

        #[arg(requires = "kind")]
        name: Option<String>,
    },
    List {
        #[arg(value_enum)]
        kind: SchemaKind,
    },
    Validate,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum SchemaKind {
    Flavour,
    Relation,
}

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
enum CliTemplate {
    #[default]
    Minimal,
    Empty,
}

struct ValidationContext {
    project: mara::Project,
    corpus: mara::Corpus,
    schema: Option<Schema>,
    diagnostics: Vec<Diagnostic>,
    project_errors: Vec<String>,
    schema_errors: Vec<String>,
}

impl From<CliTemplate> for Template {
    fn from(value: CliTemplate) -> Self {
        match value {
            CliTemplate::Minimal => Self::Minimal,
            CliTemplate::Empty => Self::Empty,
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), String> {
    let Cli { project, command } = cli;
    match command {
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
            let project =
                initialize_project(target, template.into()).map_err(|error| error.to_string())?;
            println!("initialized Mara project at {}", project.root().display());
            println!("created {PROJECT_FILE}", PROJECT_FILE = mara::PROJECT_FILE);
            println!("created {SCHEMA_FILE}", SCHEMA_FILE = mara::SCHEMA_FILE);
            Ok(())
        }
        Command::Project {
            command: ProjectCommand::Validate,
        } => {
            let context = load_selected_project(project)?;
            report_diagnostics(context, None)
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
            let (project, schema) = load_mutation_project(project)?;
            let body = match body.as_deref() {
                Some("-") => {
                    let mut body = String::new();
                    io::stdin()
                        .read_to_string(&mut body)
                        .map_err(|error| format!("could not read item body from stdin: {error}"))?;
                    Some(body)
                }
                _ => body,
            };
            let created = create_item(
                &project,
                &schema,
                ItemCreationRequest {
                    flavour,
                    id: id.clone(),
                    file,
                    title,
                    fields: fields
                        .into_iter()
                        .map(|field| (field.key, field.value))
                        .collect(),
                    body,
                    line,
                },
            )
            .map_err(|error| error.to_string())?;
            println!(
                "created item '{}' at {}:{}",
                id,
                created.path().display(),
                created.line()
            );
            println!("complete: {}", created.is_complete());
            if !created.is_complete() {
                println!("missing: body");
            }
            Ok(())
        }
        Command::Item {
            command: ItemCommand::Validate { id },
        } => {
            let context = load_selected_project(project)?;
            report_diagnostics(context, Some(&id))
        }
        Command::Item {
            command: ItemCommand::Get { id },
        } => {
            let (corpus, _) = load_query_project(project)?;
            let item = get_item(&corpus, &id).map_err(|error| error.to_string())?;
            print_resolved_item(&item);
            Ok(())
        }
        Command::Item {
            command: ItemCommand::List { filters },
        } => {
            let (corpus, schema) = load_query_project(project)?;
            let items = list_items(&corpus, &schema, &filters.into_domain())
                .map_err(|error| error.to_string())?;
            print_item_summaries(&items);
            Ok(())
        }
        Command::Item {
            command: ItemCommand::Search { query, filters },
        } => {
            let (corpus, schema) = load_query_project(project)?;
            let items = search_items(&corpus, &schema, &query, &filters.into_domain())
                .map_err(|error| error.to_string())?;
            print_item_summaries(&items);
            Ok(())
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
            let (corpus, schema) = load_query_project(project)?;
            let filters = RelatedFilters::new(direction.map(Into::into), relation, flavour);
            let items = related_items(&corpus, &schema, &id, &filters)
                .map_err(|error| error.to_string())?;
            print_related_items(&items);
            Ok(())
        }
        Command::Relation {
            command:
                RelationCommand::Add {
                    source,
                    relation,
                    target,
                },
        } => {
            let (project, schema) = load_mutation_project(project)?;
            let mutation = add_relation(&project, &schema, &source, &relation, &target)
                .map_err(|error| error.to_string())?;
            println!(
                "added relation '{}' from '{}' to '{}' in {}",
                mutation.relation(),
                mutation.source(),
                mutation.target(),
                mutation.path().display()
            );
            Ok(())
        }
        Command::Relation {
            command:
                RelationCommand::Remove {
                    source,
                    relation,
                    target,
                },
        } => {
            let (project, schema) = load_mutation_project(project)?;
            let mutation = remove_relation(&project, &schema, &source, &relation, &target)
                .map_err(|error| error.to_string())?;
            println!(
                "removed relation '{}' from '{}' to '{}' in {}",
                mutation.relation(),
                mutation.source(),
                mutation.target(),
                mutation.path().display()
            );
            Ok(())
        }
        Command::Schema { command } => {
            let current_directory = env::current_dir()
                .map_err(|error| format!("could not read current directory: {error}"))?;
            let project = resolve_project(project.as_deref(), current_directory)
                .map_err(|error| error.to_string())?;
            let schema = load_schema(&project).map_err(|error| error.to_string())?;
            run_schema_command(command, &project, &schema)
        }
    }
}

fn load_mutation_project(selected: Option<PathBuf>) -> Result<(mara::Project, Schema), String> {
    let current_directory =
        env::current_dir().map_err(|error| format!("could not read current directory: {error}"))?;
    let project = resolve_project(selected.as_deref(), current_directory)
        .map_err(|error| error.to_string())?;
    let schema = load_schema(&project).map_err(|error| error.to_string())?;
    Ok((project, schema))
}

fn load_query_project(selected: Option<PathBuf>) -> Result<(mara::Corpus, Schema), String> {
    let (project, schema) = load_mutation_project(selected)?;
    let corpus = load_corpus(&project, &schema).map_err(|error| error.to_string())?;
    Ok((corpus, schema))
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

fn print_item_summaries(items: &[ItemSummary]) {
    for item in items {
        print_item_summary(item);
    }
}

fn print_item_heading(item: &ItemSummary) {
    println!("{}\t{}\t{}", item.id(), item.flavour(), item.title());
}

fn print_item_summary(item: &ItemSummary) {
    println!(
        "{}\t{}\t{}\t{}:{}",
        item.id(),
        item.flavour(),
        item.title(),
        item.path().display(),
        item.line()
    );
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

fn load_selected_project(selected: Option<PathBuf>) -> Result<ValidationContext, String> {
    let cwd =
        env::current_dir().map_err(|error| format!("could not read current directory: {error}"))?;
    let project_validation = resolve_project_for_validation(selected.as_deref(), cwd)
        .map_err(|error| error.to_string())?;
    let (project, project_errors, schema_available) = project_validation.into_parts();
    let project_errors = project_errors
        .into_iter()
        .map(|message| {
            format!(
                "invalid Mara project at {}: {message}",
                project.root().join(mara::PROJECT_FILE).display()
            )
        })
        .collect();
    let (schema, schema_errors, corpus, diagnostics) = if schema_available {
        match load_schema_for_validation(&project) {
            Ok((schema, errors)) if schema.format_version() == 1 => {
                let schema_errors = errors
                    .into_iter()
                    .map(|message| {
                        format!(
                            "invalid Mara schema at {}: {message}",
                            project.schema_path().display()
                        )
                    })
                    .collect();
                let (corpus, diagnostics) = load_corpus_for_validation(&project, &schema)
                    .map_err(|error| error.to_string())?;
                (Some(schema), schema_errors, corpus, diagnostics)
            }
            Ok((_, errors)) => {
                let schema_errors = errors
                    .into_iter()
                    .map(|message| {
                        format!(
                            "invalid Mara schema at {}: {message}",
                            project.schema_path().display()
                        )
                    })
                    .collect();
                let (corpus, diagnostics) = load_corpus_syntax_for_validation(&project)
                    .map_err(|error| error.to_string())?;
                (None, schema_errors, corpus, diagnostics)
            }
            Err(error) => {
                let schema_error = error.to_string();
                let (corpus, diagnostics) = load_corpus_syntax_for_validation(&project)
                    .map_err(|error| error.to_string())?;
                (None, vec![schema_error], corpus, diagnostics)
            }
        }
    } else {
        let (corpus, diagnostics) =
            load_corpus_syntax_for_validation(&project).map_err(|error| error.to_string())?;
        (None, Vec::new(), corpus, diagnostics)
    };
    Ok(ValidationContext {
        project,
        corpus,
        schema,
        diagnostics,
        project_errors,
        schema_errors,
    })
}

fn report_diagnostics(
    mut context: ValidationContext,
    selected: Option<&str>,
) -> Result<(), String> {
    match &context.schema {
        Some(schema) => context
            .diagnostics
            .extend(validate_corpus(&context.corpus, schema)),
        None => context
            .diagnostics
            .extend(validate_corpus_independent(&context.corpus)),
    }
    let selected_item_missing = selected.is_some_and(|id| {
        context.corpus.is_complete()
            && !context.corpus.items().any(|item| item.id() == id)
            && !context
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.applies_to_item(id))
    });
    let selected_item_context_incomplete = selected.is_some() && !context.corpus.is_complete();
    let diagnostics = context
        .diagnostics
        .into_iter()
        .filter(|diagnostic| {
            selected.is_none_or(|id| {
                diagnostic.applies_to_item(id)
                    || context
                        .corpus
                        .items()
                        .filter(|item| item.id() == id)
                        .any(|item| {
                            diagnostic.source().span().start_byte()
                                >= item.source().span().start_byte()
                                && diagnostic.source().span().end_byte()
                                    <= item.source().span().end_byte()
                                && diagnostic.source().path() == item.source().path()
                        })
            })
        })
        .collect::<Vec<_>>();
    let diagnostic_count = diagnostics.len()
        + context.project_errors.len()
        + context.schema_errors.len()
        + usize::from(selected_item_missing)
        + usize::from(selected_item_context_incomplete);
    if diagnostic_count == 0 {
        println!(
            "valid {}",
            selected.map_or_else(
                || format!("project at {}", context.project.root().display()),
                |id| format!("item '{id}'")
            )
        );
        return Ok(());
    }
    for error in context.project_errors {
        eprintln!("error: {error}");
    }
    for error in context.schema_errors {
        eprintln!("error: {error}");
    }
    if selected_item_context_incomplete {
        eprintln!(
            "error: item '{}' could not be fully validated because the project corpus is incomplete",
            selected.expect("incomplete selected-item validation has an item ID")
        );
    }
    if selected_item_missing {
        eprintln!(
            "error: item '{}' was not found",
            selected.expect("missing selected item has an item ID")
        );
    }
    for diagnostic in &diagnostics {
        eprintln!(
            "{}:{}: error: {}",
            diagnostic.source().path().display(),
            diagnostic.source().span().start_line(),
            diagnostic.message()
        );
    }
    Err(format!(
        "validation failed with {} diagnostic{}",
        diagnostic_count,
        if diagnostic_count == 1 { "" } else { "s" }
    ))
}

fn run_schema_command(
    command: SchemaCommand,
    project: &mara::Project,
    schema: &Schema,
) -> Result<(), String> {
    match command {
        SchemaCommand::Get {
            kind: None,
            name: None,
        } => print_yaml(schema),
        SchemaCommand::Get {
            kind: Some(SchemaKind::Flavour),
            name: Some(name),
        } => {
            let definition = schema
                .flavours()
                .get(&name)
                .ok_or_else(|| format!("unknown flavour '{name}'"))?;
            print_named_yaml(&name, definition)
        }
        SchemaCommand::Get {
            kind: Some(SchemaKind::Relation),
            name: Some(name),
        } => {
            let definition = schema
                .relations()
                .get(&name)
                .ok_or_else(|| format!("unknown relation '{name}'"))?;
            print_named_yaml(&name, definition)
        }
        SchemaCommand::Get { .. } => {
            Err("schema get requires both KIND and NAME, or neither".into())
        }
        SchemaCommand::List {
            kind: SchemaKind::Flavour,
        } => {
            for (name, definition) in schema.flavours() {
                println!("{name}\t{}", definition.description());
            }
            Ok(())
        }
        SchemaCommand::List {
            kind: SchemaKind::Relation,
        } => {
            for (name, definition) in schema.relations() {
                println!("{name}\t{}", definition.description());
            }
            Ok(())
        }
        SchemaCommand::Validate => {
            println!(
                "valid schema at {} ({} flavours, {} relations)",
                project.schema_path().display(),
                schema.flavours().len(),
                schema.relations().len()
            );
            Ok(())
        }
    }
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
