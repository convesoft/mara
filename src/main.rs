use std::{path::PathBuf, process::ExitCode};

use clap::{Parser, Subcommand, ValueEnum};
use mara::{Template, initialize_project};

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
}

#[derive(Debug, Subcommand)]
enum ProjectCommand {
    Init {
        path: Option<PathBuf>,

        #[arg(long, value_enum, default_value_t)]
        template: CliTemplate,
    },
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
    match cli.command {
        Command::Project {
            command: ProjectCommand::Init { path, template },
        } => {
            let target = match (cli.project, path) {
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
    }
}
