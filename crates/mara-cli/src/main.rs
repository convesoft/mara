use std::{path::PathBuf, process::ExitCode};

use clap::{Parser, Subcommand, ValueEnum};
use mara_engine::command::{
    CommandOutput, OutputFormat, generate_project_mid, initialize_project, run_check, run_list,
    run_show, run_trace,
};

#[derive(Debug, Parser)]
#[command(
    name = "mara",
    version,
    about = "Mara bootstrap command-line interface"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Init {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long, default_value = "mara-project")]
        name: String,
    },
    Mid,
    Schema {
        #[command(subcommand)]
        command: SchemaCommand,
    },
    Check {
        #[arg(long, value_enum, default_value_t)]
        format: Format,
    },
    List {
        #[arg(long, value_enum, default_value_t)]
        format: Format,
        #[arg(long)]
        flavour: Vec<String>,
        #[arg(long)]
        field: Vec<String>,
    },
    Show {
        reference: String,
        #[arg(long, value_enum, default_value_t)]
        format: Format,
    },
    Trace {
        reference: String,
        #[arg(long, value_enum, default_value_t)]
        format: Format,
        #[arg(long, value_enum, default_value_t)]
        direction: Direction,
        #[arg(long, default_value_t = 1)]
        depth: usize,
    },
}

#[derive(Debug, Subcommand)]
enum SchemaCommand {
    Check {
        #[arg(long, value_enum, default_value_t)]
        format: Format,
    },
}

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
enum Format {
    #[default]
    Human,
    Json,
}

impl From<Format> for OutputFormat {
    fn from(value: Format) -> Self {
        match value {
            Format::Human => Self::Human,
            Format::Json => Self::Json,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
enum Direction {
    Incoming,
    #[default]
    Outgoing,
    Bidirectional,
}

impl From<Direction> for mara_engine::command::TraceDirection {
    fn from(value: Direction) -> Self {
        match value {
            Direction::Incoming => Self::Incoming,
            Direction::Outgoing => Self::Outgoing,
            Direction::Bidirectional => Self::Bidirectional,
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Init { path, name } => match initialize_project(path, &name) {
            Ok(_) => {
                print!("created .mara/project.toml\ncreated .mara/schema.yaml\n");
                ExitCode::SUCCESS
            }
            Err(error) => {
                print!("{}", error.render_human());
                ExitCode::from(2)
            }
        },
        Command::Mid => match generate_project_mid(".") {
            Ok(mid) => {
                println!("{}", mid.as_str());
                ExitCode::SUCCESS
            }
            Err(error) => {
                print!("{}", error.render_human());
                ExitCode::from(2)
            }
        },
        Command::Schema {
            command: SchemaCommand::Check { format },
        } => emit(run_check(".", true), format),
        Command::Check { format } => emit(run_check(".", false), format),
        Command::List {
            format,
            flavour,
            field,
        } => emit(run_list(".", &flavour, &field), format),
        Command::Show { reference, format } => emit(run_show(".", &reference), format),
        Command::Trace {
            reference,
            format,
            direction,
            depth,
        } => emit(run_trace(".", &reference, direction.into(), depth), format),
    }
}

fn emit(output: CommandOutput, format: Format) -> ExitCode {
    let status = output.status();
    print!("{}", output.render(format.into()));
    ExitCode::from(status.exit_code())
}
