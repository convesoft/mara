use std::{collections::BTreeMap, env, path::PathBuf, process::ExitCode};

use clap::{Parser, Subcommand, ValueEnum};
use mara::{Schema, Template, initialize_project, load_schema, resolve_project};
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
}

#[derive(Debug, Subcommand)]
enum ProjectCommand {
    Init {
        path: Option<PathBuf>,

        #[arg(long, value_enum, default_value_t)]
        template: CliTemplate,
    },
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
