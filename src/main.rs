use std::{collections::BTreeMap, env, path::PathBuf, process::ExitCode};

use clap::{Parser, Subcommand, ValueEnum};
use mara::{
    Diagnostic, Schema, Template, initialize_project, load_corpus_for_validation,
    load_corpus_syntax_for_validation, load_schema, load_schema_for_validation, resolve_project,
    resolve_project_for_validation, validate_corpus, validate_corpus_independent,
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
    Validate { id: String },
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
            command: ItemCommand::Validate { id },
        } => {
            let context = load_selected_project(project)?;
            report_diagnostics(context, Some(&id))
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
