use clap::{Parser, Subcommand, ValueEnum, error::ErrorKind};
use mara::{
    OperationContext, ProjectInitializationResult, SchemaGetResult, SchemaKind, SchemaListResult,
    Template, project_initialize,
};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    env,
    ffi::OsString,
    io::{self, Write},
    path::PathBuf,
    process::ExitCode,
};
mod mcp;

#[derive(Debug, Parser)]
#[command(
    name = "mara",
    version,
    about = "Structured project knowledge",
    after_help = "This rebuild checkpoint supports project initialization and schema inspection. Further capabilities are pending their implementation reviews."
)]
struct Cli {
    /// Use this project root instead of ancestor discovery; selects the init target or binds MCP.
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
    /// Initialize a Mara project.
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    /// Inspect the selected project's schema declarations.
    Schema {
        #[command(subcommand)]
        command: SchemaCommand,
    },
    /// Serve the available operations over stdio MCP; resolve projects per call.
    Mcp,
}

#[derive(Debug, Subcommand)]
enum ProjectCommand {
    /// Initialize the current or named directory without overwriting existing files.
    Init {
        /// Destination; omit to use --project or the current directory. Cannot combine with --project.
        path: Option<PathBuf>,
        /// Bundled schema: minimal (default), empty, or engineering with policy/check files.
        #[arg(long, value_enum, default_value_t)]
        template: CliTemplate,
    },
}

#[derive(Debug, Subcommand)]
enum SchemaCommand {
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
        Command::Mcp => mcp::run(project)?,
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
