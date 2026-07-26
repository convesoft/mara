//! Reusable bootstrap command services and deterministic presentation models.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

pub use mara_core::TraceDirection;

use mara_core::{
    AuthoredReference, AuthoredReferenceSyntax, Diagnostic, DiagnosticValue, FieldDiagnosticCode,
    ItemDiagnosticCode, Mid, NodeRef, NormalizedItem, NormalizedScalar, ReferenceDiagnosticCode,
    SchemaDocument, SourceSpan, TraceResult,
};
use mara_markdown::{ParsedDocument, ParsedItem};
use serde::Serialize;

use crate::{
    ValidationResult, check_project, check_schema, compile_scalar,
    identity::generate_mid,
    index::{IndexError, write_index},
    project::{
        LoadedProject, ProjectLoadError, ProjectLoadOperationalErrorCode, discover_and_load,
    },
    schema::load_schema,
    semantic::{relation_inverse_wire_name, relation_occurrence_wire_origin},
};

const PROJECT_FILE: &str = ".mara/project.toml";
const SCHEMA_FILE: &str = ".mara/schema.yaml";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Human,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandName {
    Check,
    SchemaCheck,
    List,
    Show,
    Trace,
    Index,
}

impl CommandName {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Check => "check",
            Self::SchemaCheck => "schema_check",
            Self::List => "list",
            Self::Show => "show",
            Self::Trace => "trace",
            Self::Index => "index",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandStatus {
    Ok,
    Invalid,
    Failed,
}

impl CommandStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Invalid => "invalid",
            Self::Failed => "failed",
        }
    }

    pub const fn exit_code(self) -> u8 {
        match self {
            Self::Ok => 0,
            Self::Invalid => 1,
            Self::Failed => 2,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectWire {
    name: String,
    root: &'static str,
    schema_name: String,
    schema_version: String,
    schema_path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceSpanWire {
    path: String,
    start_byte: u64,
    end_byte: u64,
    start_line: u64,
    start_column: u64,
    end_line: u64,
    end_column: u64,
}

impl From<&SourceSpan> for SourceSpanWire {
    fn from(span: &SourceSpan) -> Self {
        Self {
            path: span.path().to_owned(),
            start_byte: span.start_byte(),
            end_byte: span.end_byte(),
            start_line: span.start_line(),
            start_column: span.start_column(),
            end_line: span.end_line(),
            end_column: span.end_column(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct RelatedDiagnosticWire {
    message: String,
    span: SourceSpanWire,
}

#[derive(Debug, Clone, Serialize)]
struct DiagnosticItemWire {
    mid: String,
    id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct DiagnosticContextWire {
    field: Option<String>,
    relation: Option<String>,
    target: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticWire {
    code: String,
    severity: String,
    message: String,
    primary: Option<SourceSpanWire>,
    related: Vec<RelatedDiagnosticWire>,
    item: Option<DiagnosticItemWire>,
    context: DiagnosticContextWire,
    details: BTreeMap<String, serde_json::Value>,
}

impl DiagnosticWire {
    fn invalid_argument(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_owned(),
            severity: "error".to_owned(),
            message: message.into(),
            primary: None,
            related: Vec::new(),
            item: None,
            context: DiagnosticContextWire {
                field: None,
                relation: None,
                target: None,
            },
            details: BTreeMap::new(),
        }
    }
}

impl From<&Diagnostic> for DiagnosticWire {
    fn from(diagnostic: &Diagnostic) -> Self {
        Self {
            code: diagnostic.code().as_str().to_owned(),
            severity: diagnostic.severity().as_str().to_owned(),
            message: diagnostic.message().to_owned(),
            primary: diagnostic.primary().map(SourceSpanWire::from),
            related: diagnostic
                .related()
                .iter()
                .map(|related| RelatedDiagnosticWire {
                    message: related.message().to_owned(),
                    span: SourceSpanWire::from(related.span()),
                })
                .collect(),
            item: diagnostic.item().map(|item| DiagnosticItemWire {
                mid: item.mid().as_str().to_owned(),
                id: item.id().map(str::to_owned),
            }),
            context: DiagnosticContextWire {
                field: diagnostic.context().field().map(str::to_owned),
                relation: diagnostic.context().relation().map(str::to_owned),
                target: diagnostic.context().target().map(str::to_owned),
            },
            details: diagnostic
                .details()
                .iter()
                .map(|(key, value)| (key.clone(), diagnostic_value(value)))
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct OperationalErrorWire {
    code: String,
    message: String,
    details: BTreeMap<String, serde_json::Value>,
}

impl OperationalErrorWire {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: BTreeMap::new(),
        }
    }

    pub fn with_detail(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.details
            .insert(key.into(), serde_json::Value::String(value.into()));
        self
    }

    pub fn render_human(&self) -> String {
        format!("error[{}]: {}\n", self.code, self.message)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SummaryWire {
    documents: usize,
    items: usize,
    source_nodes: usize,
    edges: usize,
    mentions: usize,
    external_nodes: usize,
    errors: usize,
    warnings: usize,
    info: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ItemSummaryWire {
    mid: String,
    id: Option<String>,
    flavour: String,
    title: Option<String>,
    source: SourceSpanWire,
}

#[derive(Debug, Clone, Serialize)]
pub struct CompiledFilterWire {
    flavour: String,
    #[serde(rename = "type")]
    field_type: String,
    values: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FieldFilterWire {
    name: String,
    raw_values: Vec<String>,
    compiled: Vec<CompiledFilterWire>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ListFiltersWire {
    flavours: Vec<String>,
    fields: Vec<FieldFilterWire>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CheckData {
    summary: SummaryWire,
}

#[derive(Debug, Clone, Serialize)]
pub struct ListData {
    filters: ListFiltersWire,
    items: Vec<ItemSummaryWire>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ShowData {
    item: ItemWire,
}

#[derive(Debug, Clone, Serialize)]
pub struct TraceData {
    focus_mid: String,
    direction: String,
    max_depth: usize,
    nodes: Vec<TraceNodeWire>,
    paths: Vec<TracePathWire>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IndexData {
    path: String,
    sha256: String,
    summary: SummaryWire,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum CommandData {
    Check(CheckData),
    List(ListData),
    Show(Box<ShowData>),
    Trace(TraceData),
    Index(IndexData),
}

#[derive(Debug, Clone, Serialize)]
pub struct CommandOutput {
    format: &'static str,
    version: u8,
    command: &'static str,
    status: &'static str,
    project: Option<ProjectWire>,
    diagnostics: Vec<DiagnosticWire>,
    data: Option<CommandData>,
    error: Option<OperationalErrorWire>,
}

impl CommandOutput {
    fn ok(
        command: CommandName,
        project: ProjectWire,
        diagnostics: Vec<DiagnosticWire>,
        data: CommandData,
    ) -> Self {
        Self::new(
            command,
            CommandStatus::Ok,
            Some(project),
            diagnostics,
            Some(data),
            None,
        )
    }

    fn invalid(
        command: CommandName,
        project: Option<ProjectWire>,
        diagnostics: Vec<DiagnosticWire>,
    ) -> Self {
        Self::new(
            command,
            CommandStatus::Invalid,
            project,
            diagnostics,
            None,
            None,
        )
    }

    pub fn failed(command: CommandName, error: OperationalErrorWire) -> Self {
        Self::new(
            command,
            CommandStatus::Failed,
            None,
            Vec::new(),
            None,
            Some(error),
        )
    }

    fn failed_with_project(
        command: CommandName,
        project: ProjectWire,
        diagnostics: Vec<DiagnosticWire>,
        error: OperationalErrorWire,
    ) -> Self {
        Self::new(
            command,
            CommandStatus::Failed,
            Some(project),
            diagnostics,
            None,
            Some(error),
        )
    }

    fn new(
        command: CommandName,
        status: CommandStatus,
        project: Option<ProjectWire>,
        diagnostics: Vec<DiagnosticWire>,
        data: Option<CommandData>,
        error: Option<OperationalErrorWire>,
    ) -> Self {
        Self {
            format: "mara.command",
            version: 1,
            command: command.as_str(),
            status: status.as_str(),
            project,
            diagnostics,
            data,
            error,
        }
    }

    pub fn status(&self) -> CommandStatus {
        match self.status {
            "ok" => CommandStatus::Ok,
            "invalid" => CommandStatus::Invalid,
            "failed" => CommandStatus::Failed,
            _ => unreachable!("command status is a closed set"),
        }
    }

    pub fn render(&self, format: OutputFormat) -> String {
        match format {
            OutputFormat::Json => {
                let mut rendered = serde_json::to_string_pretty(self)
                    .expect("Mara command wire values are serializable");
                rendered.push('\n');
                rendered
            }
            OutputFormat::Human => self.render_human(),
        }
    }

    fn render_human(&self) -> String {
        if let Some(error) = &self.error {
            return format!("error[{}]: {}\n", error.code, error.message);
        }
        let diagnostics = self.render_human_diagnostics();
        if self.status == "invalid" {
            return diagnostics;
        }
        let mut output = match self.data.as_ref() {
            Some(CommandData::Check(data)) => format!(
                "ok: {} documents, {} items, {} edges; 0 errors, {} warnings, {} info\n",
                data.summary.documents,
                data.summary.items,
                data.summary.edges,
                data.summary.warnings,
                data.summary.info
            ),
            Some(CommandData::List(data)) => data
                .items
                .iter()
                .map(|item| {
                    format!(
                        "{}\t{}\t{}\t{}\t{}:{}:{}\n",
                        item.mid,
                        item.id.as_deref().unwrap_or("-"),
                        item.flavour,
                        item.title.as_deref().unwrap_or("-"),
                        item.source.path,
                        item.source.start_line,
                        item.source.start_column
                    )
                })
                .collect(),
            Some(CommandData::Show(data)) => data.item.render_human(),
            Some(CommandData::Trace(data)) => data.render_human(),
            Some(CommandData::Index(data)) => {
                format!("wrote {} ({})\n", data.path, data.sha256)
            }
            None => String::new(),
        };
        output.push_str(&diagnostics);
        output
    }

    fn render_human_diagnostics(&self) -> String {
        if self.diagnostics.is_empty() {
            return String::new();
        }
        let mut output = String::new();
        let mut errors = 0;
        let mut warnings = 0;
        let mut info = 0;
        for diagnostic in &self.diagnostics {
            match diagnostic.severity.as_str() {
                "error" => errors += 1,
                "warning" => warnings += 1,
                "info" => info += 1,
                _ => unreachable!("diagnostic severity is a closed set"),
            }
            if let Some(primary) = &diagnostic.primary {
                output.push_str(&format!(
                    "{}:{}:{}: {}[{}]: {}\n",
                    primary.path,
                    primary.start_line,
                    primary.start_column,
                    diagnostic.severity,
                    diagnostic.code,
                    diagnostic.message
                ));
            } else {
                output.push_str(&format!(
                    "<project>: {}[{}]: {}\n",
                    diagnostic.severity, diagnostic.code, diagnostic.message
                ));
            }
        }
        output.push_str(&format!(
            "{errors} errors, {warnings} warnings, {info} info\n"
        ));
        output
    }
}

#[derive(Debug, Clone)]
pub struct InitResult {
    pub root: PathBuf,
    pub project_file: PathBuf,
    pub schema_file: PathBuf,
}

pub fn initialize_project(
    target: impl AsRef<Path>,
    name: &str,
) -> Result<InitResult, OperationalErrorWire> {
    if !valid_schema_name(name) {
        return Err(OperationalErrorWire::new(
            "cli.invalid_arguments",
            "project name must start with a lowercase letter and contain only lowercase letters, digits, '_' or '-'",
        ));
    }
    let target = target.as_ref();
    fs::create_dir_all(target).map_err(|_| {
        OperationalErrorWire::new("io.failed", "could not create the target directory")
    })?;
    let root = fs::canonicalize(target).map_err(|_| {
        OperationalErrorWire::new("io.failed", "could not resolve the target directory")
    })?;
    let mara_dir = root.join(".mara");
    let project_file = root.join(PROJECT_FILE);
    let schema_file = root.join(SCHEMA_FILE);
    if project_file.exists() || schema_file.exists() {
        return Err(OperationalErrorWire::new(
            "io.failed",
            "refusing to overwrite an existing Mara project or target file",
        ));
    }
    fs::create_dir_all(&mara_dir).map_err(|_| {
        OperationalErrorWire::new("io.failed", "could not create the .mara directory")
    })?;
    write_new(&project_file, &initial_project_toml(name))?;
    write_new(&schema_file, &initial_schema_yaml(name))?;
    Ok(InitResult {
        root,
        project_file,
        schema_file,
    })
}

pub fn generate_project_mid(start: impl AsRef<Path>) -> Result<Mid, OperationalErrorWire> {
    let project = discover_and_load(start).map_err(|_| {
        OperationalErrorWire::new(
            "project.unavailable",
            "a valid Mara project and schema are required to generate a MID",
        )
    })?;
    let schema = load_schema(&project).map_err(|_| {
        OperationalErrorWire::new(
            "project.unavailable",
            "a valid Mara project and schema are required to generate a MID",
        )
    })?;
    generate_mid(schema.identity().value().mid().value())
        .map_err(|_| OperationalErrorWire::new("internal.failed", "could not generate a MID"))
}

pub fn run_check(start: impl AsRef<Path>, schema_only: bool) -> CommandOutput {
    let command = if schema_only {
        CommandName::SchemaCheck
    } else {
        CommandName::Check
    };
    let result = if schema_only {
        check_schema(start)
    } else {
        check_project(start)
    };
    match result {
        Ok(result) => validation_output(command, result),
        Err(error) => project_load_output(command, error),
    }
}

pub fn run_list(start: impl AsRef<Path>, flavours: &[String], fields: &[String]) -> CommandOutput {
    let result = match check_project(start) {
        Ok(result) => result,
        Err(error) => return project_load_output(CommandName::List, error),
    };
    if !result.is_valid() {
        return validation_invalid(CommandName::List, &result);
    }
    let Some((project, schema, semantic)) = query_inputs(&result) else {
        return CommandOutput::failed(
            CommandName::List,
            OperationalErrorWire::new("internal.failed", "query model is unavailable"),
        );
    };
    let filters = match compile_filters(schema, flavours, fields) {
        Ok(filters) => filters,
        Err(diagnostics) => {
            return CommandOutput::invalid(
                CommandName::List,
                Some(project_wire(project, schema)),
                diagnostics,
            );
        }
    };
    let items = semantic
        .items()
        .iter()
        .filter(|item| filters.matches(item))
        .map(ItemSummaryWire::from)
        .collect();
    CommandOutput::ok(
        CommandName::List,
        project_wire(project, schema),
        result
            .diagnostics()
            .iter()
            .map(DiagnosticWire::from)
            .collect(),
        CommandData::List(ListData {
            filters: filters.wire,
            items,
        }),
    )
}

pub fn run_show(start: impl AsRef<Path>, reference: &str) -> CommandOutput {
    let result = match check_project(start) {
        Ok(result) => result,
        Err(error) => return project_load_output(CommandName::Show, error),
    };
    if let Some(output) = ambiguous_query_output(CommandName::Show, &result, reference) {
        return output;
    }
    if !result.is_valid() {
        return validation_invalid(CommandName::Show, &result);
    }
    let Some((project, schema, semantic)) = query_inputs(&result) else {
        return CommandOutput::failed(
            CommandName::Show,
            OperationalErrorWire::new("internal.failed", "query model is unavailable"),
        );
    };
    let mid = match resolve_reference(schema, semantic.identity_index(), reference) {
        Ok(mid) => mid,
        Err(diagnostic) => {
            return CommandOutput::invalid(
                CommandName::Show,
                Some(project_wire(project, schema)),
                vec![*diagnostic],
            );
        }
    };
    let item = semantic
        .items()
        .iter()
        .find(|item| item.mid() == &mid)
        .expect("resolved MID belongs to a normalized item");
    let parsed = parsed_item(result.documents(), &mid)
        .expect("a normalized item retains its parsed source item");
    let wire = ItemWire::new(item, parsed, schema, semantic);
    CommandOutput::ok(
        CommandName::Show,
        project_wire(project, schema),
        result
            .diagnostics()
            .iter()
            .map(DiagnosticWire::from)
            .collect(),
        CommandData::Show(Box::new(ShowData { item: wire })),
    )
}

pub fn run_trace(
    start: impl AsRef<Path>,
    reference: &str,
    direction: TraceDirection,
    max_depth: usize,
) -> CommandOutput {
    let result = match check_project(start) {
        Ok(result) => result,
        Err(error) => return project_load_output(CommandName::Trace, error),
    };
    if let Some(output) = ambiguous_query_output(CommandName::Trace, &result, reference) {
        return output;
    }
    if !result.is_valid() {
        return validation_invalid(CommandName::Trace, &result);
    }
    let Some((project, schema, semantic)) = query_inputs(&result) else {
        return CommandOutput::failed(
            CommandName::Trace,
            OperationalErrorWire::new("internal.failed", "query model is unavailable"),
        );
    };
    if max_depth == 0 {
        return CommandOutput::failed(
            CommandName::Trace,
            OperationalErrorWire::new("cli.invalid_arguments", "trace depth must be positive"),
        );
    }
    let mid = match resolve_reference(schema, semantic.identity_index(), reference) {
        Ok(mid) => mid,
        Err(diagnostic) => {
            return CommandOutput::invalid(
                CommandName::Trace,
                Some(project_wire(project, schema)),
                vec![*diagnostic],
            );
        }
    };
    let Some(graph) = result.graph() else {
        return CommandOutput::failed(
            CommandName::Trace,
            OperationalErrorWire::new("internal.failed", "query graph is unavailable"),
        );
    };
    let traced = graph
        .trace(&NodeRef::item(mid.clone()), direction, None, max_depth)
        .expect("validated focus and positive depth satisfy trace prerequisites");
    let data = TraceData::new(&traced, semantic.items());
    CommandOutput::ok(
        CommandName::Trace,
        project_wire(project, schema),
        result
            .diagnostics()
            .iter()
            .map(DiagnosticWire::from)
            .collect(),
        CommandData::Trace(data),
    )
}

/// Validates the complete project and atomically writes its configured JSON index.
pub fn run_index(start: impl AsRef<Path>) -> CommandOutput {
    let result = match check_project(start) {
        Ok(result) => result,
        Err(error) => return project_load_output(CommandName::Index, error),
    };
    if !result.is_valid() {
        return validation_invalid(CommandName::Index, &result);
    }
    let Some(project) = result.project() else {
        return CommandOutput::failed(
            CommandName::Index,
            OperationalErrorWire::new("internal.failed", "project model is unavailable"),
        );
    };
    let Some(schema) = result.schema() else {
        return CommandOutput::failed(
            CommandName::Index,
            OperationalErrorWire::new("internal.failed", "schema model is unavailable"),
        );
    };
    let project_wire = project_wire(project, schema);
    let diagnostics = result
        .diagnostics()
        .iter()
        .map(DiagnosticWire::from)
        .collect::<Vec<_>>();
    match write_index(&result) {
        Ok(written) => CommandOutput::ok(
            CommandName::Index,
            project_wire,
            diagnostics,
            CommandData::Index(IndexData {
                path: written.path().to_owned(),
                sha256: written.sha256().to_owned(),
                summary: summary(&result),
            }),
        ),
        Err(error) => CommandOutput::failed_with_project(
            CommandName::Index,
            project_wire,
            diagnostics,
            operational_index_error(&error, project),
        ),
    }
}

fn operational_index_error(error: &IndexError, project: &LoadedProject) -> OperationalErrorWire {
    let wire = OperationalErrorWire::new(error.command_code(), error.command_message());
    match error.project_relative_path(&project.root) {
        Some(path) => wire.with_detail("path", path),
        None => wire,
    }
}

fn validation_output(command: CommandName, result: ValidationResult) -> CommandOutput {
    if !result.is_valid() {
        return validation_invalid(command, &result);
    }
    let Some(project) = result.project() else {
        return CommandOutput::failed(
            command,
            OperationalErrorWire::new("internal.failed", "project model is unavailable"),
        );
    };
    let Some(schema) = result.schema() else {
        return CommandOutput::failed(
            command,
            OperationalErrorWire::new("internal.failed", "schema model is unavailable"),
        );
    };
    CommandOutput::ok(
        command,
        project_wire(project, schema),
        result
            .diagnostics()
            .iter()
            .map(DiagnosticWire::from)
            .collect(),
        CommandData::Check(CheckData {
            summary: summary(&result),
        }),
    )
}

fn validation_invalid(command: CommandName, result: &ValidationResult) -> CommandOutput {
    CommandOutput::invalid(
        command,
        result
            .project()
            .zip(result.schema())
            .map(|(project, schema)| project_wire(project, schema)),
        result
            .diagnostics()
            .iter()
            .map(DiagnosticWire::from)
            .collect(),
    )
}

fn project_load_output(command: CommandName, error: ProjectLoadError) -> CommandOutput {
    match &error {
        ProjectLoadError::InvalidConfiguration {
            class,
            field,
            message,
            location,
            ..
        } if error.diagnostic_code().is_some() => {
            let primary = project_location_span(*location);
            let mut diagnostic =
                DiagnosticWire::invalid_argument(class.as_str(), message.to_string());
            diagnostic.primary = primary.as_ref().map(SourceSpanWire::from);
            diagnostic.context.field = field.map(str::to_owned);
            CommandOutput::invalid(command, None, vec![diagnostic])
        }
        ProjectLoadError::UnsafePath {
            class,
            field,
            reason,
            location,
            ..
        } if error.diagnostic_code().is_some() => {
            let primary = project_location_span(*location);
            let mut diagnostic =
                DiagnosticWire::invalid_argument(class.as_str(), reason.to_string());
            diagnostic.primary = primary.as_ref().map(SourceSpanWire::from);
            diagnostic.context.field = Some((*field).to_owned());
            CommandOutput::invalid(command, None, vec![diagnostic])
        }
        _ => CommandOutput::failed(command, operational_project_error(&error)),
    }
}

fn project_location_span(location: Option<crate::project::SourceLocation>) -> Option<SourceSpan> {
    location.and_then(|location| {
        let line_breaks = location.line.saturating_sub(1);
        let column_prefix = location.column.saturating_sub(1);
        let minimum_length = line_breaks + column_prefix;
        let leading_padding = location.byte_offset.saturating_sub(minimum_length);
        let synthetic_source = format!(
            "{}{}{}",
            " ".repeat(leading_padding),
            "\n".repeat(line_breaks),
            " ".repeat(column_prefix)
        );
        SourceSpan::try_new(
            PROJECT_FILE,
            &synthetic_source,
            location.byte_offset as u64,
            location.byte_offset as u64,
            location.line as u64,
            location.column as u64,
            location.line as u64,
            location.column as u64,
        )
        .ok()
    })
}

fn operational_project_error(error: &ProjectLoadError) -> OperationalErrorWire {
    let code = match error.operational_code() {
        Some(ProjectLoadOperationalErrorCode::IoFailed) => "io.failed",
        Some(ProjectLoadOperationalErrorCode::ProjectUnavailable) | None => "project.unavailable",
    };
    let message = match error {
        ProjectLoadError::ProjectNotFound { .. } => "no .mara/project.toml was found",
        ProjectLoadError::Io { .. } => "a required project file could not be read",
        ProjectLoadError::InvalidConfiguration { .. } | ProjectLoadError::UnsafePath { .. } => {
            "the Mara project is unavailable"
        }
    };
    OperationalErrorWire::new(code, message)
}

fn query_inputs(
    result: &ValidationResult,
) -> Option<(&LoadedProject, &SchemaDocument, &crate::SemanticCompilation)> {
    Some((result.project()?, result.schema()?, result.semantic()?))
}

fn ambiguous_query_output(
    command: CommandName,
    result: &ValidationResult,
    reference: &str,
) -> Option<CommandOutput> {
    let (project, schema, semantic) = query_inputs(result)?;
    let diagnostic = resolve_reference(schema, semantic.identity_index(), reference).err()?;
    if diagnostic.code != ReferenceDiagnosticCode::Ambiguous.as_str() {
        return None;
    }
    Some(CommandOutput::invalid(
        command,
        Some(project_wire(project, schema)),
        vec![*diagnostic],
    ))
}

fn project_wire(project: &LoadedProject, schema: &SchemaDocument) -> ProjectWire {
    ProjectWire {
        name: project.name.clone(),
        root: ".",
        schema_name: schema.schema().value().name().value().clone(),
        schema_version: schema.schema().value().version().value().clone(),
        schema_path: project.schema_source_path.clone(),
    }
}

fn summary(result: &ValidationResult) -> SummaryWire {
    let mut source_nodes = BTreeSet::new();
    let mut external_nodes = BTreeSet::new();
    if let Some(graph) = result.graph() {
        for node in graph.nodes() {
            match node {
                NodeRef::SourceSpan { source, symbol } => {
                    source_nodes.insert((
                        source.path().to_owned(),
                        source.start_byte(),
                        source.end_byte(),
                        symbol.clone(),
                    ));
                }
                NodeRef::External { uri } => {
                    external_nodes.insert(uri.clone());
                }
                NodeRef::Item { .. } => {}
            }
        }
    }
    if let Some(semantic) = result.semantic() {
        external_nodes.extend(
            semantic
                .external_mentions()
                .iter()
                .map(|reference| reference.target().to_owned()),
        );
    }
    let counts = result.severity_counts();
    SummaryWire {
        documents: result.documents().len(),
        items: result
            .semantic()
            .map_or(0, |semantic| semantic.items().len()),
        source_nodes: source_nodes.len(),
        edges: result.graph().map_or(0, mara_core::QueryGraph::edge_count),
        mentions: result.semantic().map_or(0, |semantic| {
            semantic.relations().weak_mentions().len() + semantic.external_mentions().len()
        }),
        external_nodes: external_nodes.len(),
        errors: counts.errors(),
        warnings: counts.warnings(),
        info: counts.info(),
    }
}

impl From<&NormalizedItem> for ItemSummaryWire {
    fn from(item: &NormalizedItem) -> Self {
        Self {
            mid: item.mid().as_str().to_owned(),
            id: item.display_id().map(|value| value.value().clone()),
            flavour: item.flavour().to_owned(),
            title: item.title().map(|value| value.value().clone()),
            source: SourceSpanWire::from(item.source()),
        }
    }
}

struct CompiledFilters {
    wire: ListFiltersWire,
    values: BTreeMap<String, BTreeMap<String, Vec<NormalizedScalar>>>,
}

impl CompiledFilters {
    fn matches(&self, item: &NormalizedItem) -> bool {
        if !self.wire.flavours.is_empty()
            && self
                .wire
                .flavours
                .binary_search_by(|candidate| candidate.as_bytes().cmp(item.flavour().as_bytes()))
                .is_err()
        {
            return false;
        }
        self.values.iter().all(|(field, flavours)| {
            let Some(expected) = flavours.get(item.flavour()) else {
                return false;
            };
            item.fields().get(field).is_some_and(|authored| {
                authored
                    .iter()
                    .any(|value| expected.iter().any(|expected| value.value() == expected))
            })
        })
    }
}

fn compile_filters(
    schema: &SchemaDocument,
    flavours: &[String],
    raw_fields: &[String],
) -> Result<CompiledFilters, Vec<DiagnosticWire>> {
    let mut selected = flavours.to_vec();
    selected.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    selected.dedup();
    let mut diagnostics = Vec::new();
    for flavour in &selected {
        if schema.flavours().get(flavour).is_none() {
            diagnostics.push(DiagnosticWire::invalid_argument(
                ItemDiagnosticCode::UnknownFlavour.as_str(),
                format!("unknown flavour filter {flavour:?}"),
            ));
        }
    }
    let candidate_names = if selected.is_empty() {
        schema
            .flavours()
            .definitions()
            .keys()
            .cloned()
            .collect::<Vec<_>>()
    } else {
        selected.clone()
    };
    let mut raw_by_name = BTreeMap::<String, Vec<String>>::new();
    for option in raw_fields {
        let Some((name, value)) = option.split_once('=') else {
            diagnostics.push(DiagnosticWire::invalid_argument(
                FieldDiagnosticCode::InvalidScalar.as_str(),
                "field filters must use <name>=<value>",
            ));
            continue;
        };
        raw_by_name.entry(name.to_owned()).or_default().push(
            value
                .trim_matches(|character: char| character.is_ascii_whitespace())
                .to_owned(),
        );
    }
    let mut fields_wire = Vec::new();
    let mut compiled_values = BTreeMap::new();
    for (name, mut raw_values) in raw_by_name {
        raw_values.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
        raw_values.dedup();
        let mut compiled = Vec::new();
        let mut by_flavour = BTreeMap::new();
        let mut converted_raw_values = BTreeSet::new();
        for flavour_name in &candidate_names {
            let Some(flavour) = schema.flavours().get(flavour_name) else {
                continue;
            };
            let Some(definition) = flavour.fields().get(&name) else {
                continue;
            };
            let mut values = Vec::new();
            for raw_value in &raw_values {
                if let Some(value) = compile_scalar(definition, raw_value) {
                    converted_raw_values.insert(raw_value.clone());
                    values.push(value);
                }
            }
            values.sort_by_key(scalar_key);
            values.dedup();
            if !values.is_empty() {
                compiled.push(CompiledFilterWire {
                    flavour: flavour_name.clone(),
                    field_type: definition.field_type().value().as_str().to_owned(),
                    values: values.iter().map(scalar_value).collect(),
                });
                by_flavour.insert(flavour_name.clone(), values);
            }
        }
        if by_flavour.is_empty() {
            diagnostics.push(DiagnosticWire::invalid_argument(
                FieldDiagnosticCode::InvalidScalar.as_str(),
                format!("field filter {name:?} is unknown or has no convertible value"),
            ));
        } else {
            for raw_value in &raw_values {
                if !converted_raw_values.contains(raw_value) {
                    let mut diagnostic = DiagnosticWire::invalid_argument(
                        FieldDiagnosticCode::InvalidScalar.as_str(),
                        format!(
                            "field filter value {raw_value:?} does not convert for any candidate flavour"
                        ),
                    );
                    diagnostic.context.field = Some(name.clone());
                    diagnostic.details.insert(
                        "value".to_owned(),
                        serde_json::Value::String(raw_value.clone()),
                    );
                    diagnostics.push(diagnostic);
                }
            }
        }
        fields_wire.push(FieldFilterWire {
            name: name.clone(),
            raw_values,
            compiled,
        });
        compiled_values.insert(name, by_flavour);
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(CompiledFilters {
        wire: ListFiltersWire {
            flavours: selected,
            fields: fields_wire,
        },
        values: compiled_values,
    })
}

fn resolve_reference(
    schema: &SchemaDocument,
    index: &mara_core::IdentityIndex,
    reference: &str,
) -> Result<Mid, Box<DiagnosticWire>> {
    let identity = schema.identity().value().mid().value();
    let candidates = match Mid::parse(reference, identity) {
        Ok(mid) => index.mids().get(&mid),
        Err(_) => index.display_ids().get(reference),
    };
    match candidates {
        Some(candidates) if candidates.len() == 1 => Ok(candidates[0].mid().clone()),
        Some(candidates) if !candidates.is_empty() => {
            let mut diagnostic = DiagnosticWire::invalid_argument(
                ReferenceDiagnosticCode::Ambiguous.as_str(),
                "item reference resolves to more than one item",
            );
            diagnostic.details.insert(
                "candidate_mids".to_owned(),
                serde_json::Value::Array(
                    candidates
                        .iter()
                        .map(|candidate| {
                            serde_json::Value::String(candidate.mid().as_str().to_owned())
                        })
                        .collect(),
                ),
            );
            diagnostic.details.insert(
                "reference".to_owned(),
                serde_json::Value::String(reference.to_owned()),
            );
            diagnostic.related = candidates
                .iter()
                .map(|candidate| RelatedDiagnosticWire {
                    message: format!("candidate: {}", candidate.mid().as_str()),
                    span: SourceSpanWire::from(candidate.header_source()),
                })
                .collect();
            Err(Box::new(diagnostic))
        }
        _ => {
            let mut diagnostic = DiagnosticWire::invalid_argument(
                ReferenceDiagnosticCode::Unresolved.as_str(),
                "item reference does not resolve",
            );
            diagnostic.details.insert(
                "reference".to_owned(),
                serde_json::Value::String(reference.to_owned()),
            );
            Err(Box::new(diagnostic))
        }
    }
}

fn parsed_item<'a>(documents: &'a [ParsedDocument], mid: &Mid) -> Option<&'a ParsedItem> {
    documents
        .iter()
        .flat_map(ParsedDocument::items)
        .find(|item| item.mid() == mid)
}

#[derive(Debug, Clone, Serialize)]
pub struct MetadataWire {
    key: String,
    raw_value: String,
    source: SourceSpanWire,
}

#[derive(Debug, Clone, Serialize)]
pub struct FieldValueWire {
    value: serde_json::Value,
    source: SourceSpanWire,
}

#[derive(Debug, Clone, Serialize)]
pub struct ItemFieldWire {
    name: String,
    #[serde(rename = "type")]
    field_type: String,
    values: Vec<FieldValueWire>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind")]
pub enum NodeRefWire {
    #[serde(rename = "item")]
    Item { mid: String },
    #[serde(rename = "source_span")]
    SourceSpan {
        source: SourceSpanWire,
        symbol: Option<String>,
    },
    #[serde(rename = "external")]
    External { uri: String },
}

impl From<&NodeRef> for NodeRefWire {
    fn from(node: &NodeRef) -> Self {
        match node {
            NodeRef::Item { mid } => Self::Item {
                mid: mid.as_str().to_owned(),
            },
            NodeRef::SourceSpan { source, symbol } => Self::SourceSpan {
                source: SourceSpanWire::from(source),
                symbol: symbol.clone(),
            },
            NodeRef::External { uri } => Self::External { uri: uri.clone() },
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct EdgeOccurrenceWire {
    origin: String,
    authoring_name: String,
    source: SourceSpanWire,
}

#[derive(Debug, Clone, Serialize)]
pub struct EdgeWire {
    source: NodeRefWire,
    relation: String,
    inverse_name: Option<String>,
    target: NodeRefWire,
    occurrences: Vec<EdgeOccurrenceWire>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MentionWire {
    document: String,
    source_item_mid: Option<String>,
    target: NodeRefWire,
    label: Option<String>,
    source: SourceSpanWire,
}

#[derive(Debug, Clone, Serialize)]
pub struct ItemWire {
    mid: String,
    id: Option<String>,
    flavour: String,
    title: Option<String>,
    body_markdown: String,
    document: String,
    source: SourceSpanWire,
    header_source: SourceSpanWire,
    body_source: SourceSpanWire,
    metadata: Vec<MetadataWire>,
    fields: Vec<ItemFieldWire>,
    outgoing: Vec<EdgeWire>,
    incoming: Vec<EdgeWire>,
    mentions: Vec<MentionWire>,
}

impl ItemWire {
    fn new(
        item: &NormalizedItem,
        parsed: &ParsedItem,
        schema: &SchemaDocument,
        semantic: &crate::SemanticCompilation,
    ) -> Self {
        let fields = item
            .fields()
            .iter()
            .map(|(name, values)| {
                let definition = schema
                    .flavours()
                    .get(item.flavour())
                    .and_then(|flavour| flavour.fields().get(name))
                    .expect("normalized fields retain a schema declaration");
                ItemFieldWire {
                    name: name.clone(),
                    field_type: definition.field_type().value().as_str().to_owned(),
                    values: values
                        .iter()
                        .map(|value| FieldValueWire {
                            value: scalar_value(value.value()),
                            source: SourceSpanWire::from(value.source()),
                        })
                        .collect(),
                }
            })
            .collect();
        let mut outgoing = semantic
            .relations()
            .edges()
            .iter()
            .filter(|edge| edge.source() == item.mid())
            .map(|edge| edge_wire(edge, schema))
            .collect::<Vec<_>>();
        outgoing.extend(
            semantic
                .projection_edges()
                .iter()
                .filter(|edge| edge.source().mid() == Some(item.mid()))
                .map(|edge| projection_edge_wire(edge, item, schema)),
        );
        outgoing.sort_by_key(|edge| serde_json::to_vec(edge).expect("wire edges are serializable"));
        let incoming = semantic
            .relations()
            .edges()
            .iter()
            .filter(|edge| edge.target() == item.mid())
            .map(|edge| edge_wire(edge, schema))
            .collect();
        let mut mentions = semantic
            .relations()
            .weak_mentions()
            .iter()
            .filter_map(|mention| {
                mention_wire(
                    mention.reference().authored(),
                    mention.reference().target(),
                    item.mid(),
                )
            })
            .collect::<Vec<_>>();
        mentions.extend(
            semantic
                .external_mentions()
                .iter()
                .filter_map(|reference| external_mention_wire(reference, item.mid())),
        );
        mentions.sort_by(|left, right| {
            left.document
                .as_bytes()
                .cmp(right.document.as_bytes())
                .then_with(|| left.source.start_byte.cmp(&right.source.start_byte))
        });
        Self {
            mid: item.mid().as_str().to_owned(),
            id: item.display_id().map(|value| value.value().clone()),
            flavour: item.flavour().to_owned(),
            title: item.title().map(|value| value.value().clone()),
            body_markdown: parsed.body_markdown().to_owned(),
            document: item.source().path().to_owned(),
            source: SourceSpanWire::from(item.source()),
            header_source: SourceSpanWire::from(item.header_source()),
            body_source: SourceSpanWire::from(parsed.body_source()),
            metadata: parsed
                .metadata()
                .iter()
                .map(|entry| MetadataWire {
                    key: entry.key().to_owned(),
                    raw_value: entry.raw_value().to_owned(),
                    source: SourceSpanWire::from(entry.source()),
                })
                .collect(),
            fields,
            outgoing,
            incoming,
            mentions,
        }
    }

    fn render_human(&self) -> String {
        let mut output = format!(
            "{} {}\nflavour: {}\ntitle: {}\nsource: {}:{}:{}\n",
            self.mid,
            self.id.as_deref().unwrap_or("-"),
            self.flavour,
            self.title.as_deref().unwrap_or("-"),
            self.source.path,
            self.source.start_line,
            self.source.start_column
        );
        output.push('\n');
        output.push_str(&self.body_markdown);
        if !output.ends_with('\n') {
            output.push('\n');
        }
        output
    }
}

fn edge_wire(edge: &mara_core::CanonicalRelationEdge, schema: &SchemaDocument) -> EdgeWire {
    let inverse_name = relation_inverse_wire_name(schema, edge.relation());
    EdgeWire {
        source: NodeRefWire::Item {
            mid: edge.source().as_str().to_owned(),
        },
        relation: edge.relation().to_owned(),
        inverse_name,
        target: NodeRefWire::Item {
            mid: edge.target().as_str().to_owned(),
        },
        occurrences: edge
            .occurrences()
            .iter()
            .map(|occurrence| {
                let authored = occurrence.reference().authored();
                let origin =
                    relation_occurrence_wire_origin(occurrence.origin(), authored.syntax());
                EdgeOccurrenceWire {
                    origin: origin.to_owned(),
                    authoring_name: authored.relation().unwrap_or(edge.relation()).to_owned(),
                    source: SourceSpanWire::from(authored.source()),
                }
            })
            .collect(),
    }
}

fn projection_edge_wire(
    edge: &mara_core::ProjectionEdge,
    item: &NormalizedItem,
    schema: &SchemaDocument,
) -> EdgeWire {
    let inverse_name = relation_inverse_wire_name(schema, edge.relation());
    let occurrences = item
        .authored_references()
        .iter()
        .filter(|reference| {
            reference.relation() == Some(edge.relation())
                && edge.target().uri() == Some(reference.target())
        })
        .map(|reference| EdgeOccurrenceWire {
            origin: match reference.syntax() {
                AuthoredReferenceSyntax::Inline => "typed_inline",
                AuthoredReferenceSyntax::Metadata => "canonical_metadata",
                AuthoredReferenceSyntax::Narrative => "derived_source",
            }
            .to_owned(),
            authoring_name: reference
                .relation()
                .expect("external relation occurrence has an authoring name")
                .to_owned(),
            source: SourceSpanWire::from(reference.source()),
        })
        .collect();
    EdgeWire {
        source: NodeRefWire::from(edge.source()),
        relation: edge.relation().to_owned(),
        inverse_name,
        target: NodeRefWire::from(edge.target()),
        occurrences,
    }
}

fn mention_wire(reference: &AuthoredReference, target: &Mid, focus: &Mid) -> Option<MentionWire> {
    let source_item_mid = match reference.origin() {
        mara_core::ReferenceOrigin::Item { mid, .. } if mid == focus => {
            Some(mid.as_str().to_owned())
        }
        mara_core::ReferenceOrigin::Item { .. } => return None,
        mara_core::ReferenceOrigin::Narrative(_) => return None,
    };
    Some(MentionWire {
        document: reference.source().path().to_owned(),
        source_item_mid,
        target: NodeRefWire::Item {
            mid: target.as_str().to_owned(),
        },
        label: reference.label().map(str::to_owned),
        source: SourceSpanWire::from(reference.source()),
    })
}

fn external_mention_wire(reference: &AuthoredReference, focus: &Mid) -> Option<MentionWire> {
    let source_item_mid = match reference.origin() {
        mara_core::ReferenceOrigin::Item { mid, .. } if mid == focus => {
            Some(mid.as_str().to_owned())
        }
        mara_core::ReferenceOrigin::Item { .. } | mara_core::ReferenceOrigin::Narrative(_) => {
            return None;
        }
    };
    Some(MentionWire {
        document: reference.source().path().to_owned(),
        source_item_mid,
        target: NodeRefWire::External {
            uri: reference.target().to_owned(),
        },
        label: reference.label().map(str::to_owned),
        source: SourceSpanWire::from(reference.source()),
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind")]
pub enum TraceNodeWire {
    #[serde(rename = "item")]
    Item { item: ItemSummaryWire },
    #[serde(rename = "source_span")]
    SourceSpan {
        source: SourceSpanWire,
        symbol: Option<String>,
    },
    #[serde(rename = "external")]
    External { uri: String, scheme: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct TraceStepWire {
    relation: String,
    traversal: String,
    source: NodeRefWire,
    target: NodeRefWire,
}

#[derive(Debug, Clone, Serialize)]
pub struct TracePathWire {
    nodes: Vec<NodeRefWire>,
    edges: Vec<TraceStepWire>,
}

impl TraceData {
    fn new(result: &TraceResult, items: &[NormalizedItem]) -> Self {
        let nodes = result
            .nodes()
            .iter()
            .map(|node| match node {
                NodeRef::Item { mid } => {
                    let item = items
                        .iter()
                        .find(|item| item.mid() == mid)
                        .expect("trace item node belongs to the normalized model");
                    TraceNodeWire::Item {
                        item: ItemSummaryWire::from(item),
                    }
                }
                NodeRef::SourceSpan { source, symbol } => TraceNodeWire::SourceSpan {
                    source: SourceSpanWire::from(source),
                    symbol: symbol.clone(),
                },
                NodeRef::External { uri } => TraceNodeWire::External {
                    uri: uri.clone(),
                    scheme: uri
                        .split_once(':')
                        .map_or("", |(scheme, _)| scheme)
                        .to_owned(),
                },
            })
            .collect();
        let paths = result
            .paths()
            .iter()
            .map(|path| TracePathWire {
                nodes: path.nodes().iter().map(NodeRefWire::from).collect(),
                edges: path
                    .edges()
                    .iter()
                    .map(|edge| TraceStepWire {
                        relation: edge.relation().to_owned(),
                        traversal: edge.traversal().as_str().to_owned(),
                        source: NodeRefWire::from(edge.source()),
                        target: NodeRefWire::from(edge.target()),
                    })
                    .collect(),
            })
            .collect();
        Self {
            focus_mid: result
                .focus()
                .mid()
                .expect("CLI trace focus is an item")
                .as_str()
                .to_owned(),
            direction: result.direction().as_str().to_owned(),
            max_depth: result.max_depth(),
            nodes,
            paths,
        }
    }

    fn render_human(&self) -> String {
        let mut output = format!(
            "focus: {}\ndirection: {}\nmax depth: {}\n",
            self.focus_mid, self.direction, self.max_depth
        );
        for (index, path) in self.paths.iter().enumerate() {
            output.push_str(&format!("path {}:", index + 1));
            for edge in &path.edges {
                output.push_str(&format!(" --{}:{}--> ", edge.relation, edge.traversal));
                let traversed_endpoint = if edge.traversal == "incoming" {
                    &edge.source
                } else {
                    &edge.target
                };
                match traversed_endpoint {
                    NodeRefWire::Item { mid } => output.push_str(mid),
                    NodeRefWire::SourceSpan { source, .. } => output.push_str(&format!(
                        "{}:{}:{}",
                        source.path, source.start_line, source.start_column
                    )),
                    NodeRefWire::External { uri } => output.push_str(uri),
                }
            }
            output.push('\n');
        }
        output
    }
}

fn diagnostic_value(value: &DiagnosticValue) -> serde_json::Value {
    match value {
        DiagnosticValue::Null => serde_json::Value::Null,
        DiagnosticValue::Boolean(value) => serde_json::Value::Bool(*value),
        DiagnosticValue::Integer(value) => serde_json::Value::Number((*value).into()),
        DiagnosticValue::Unsigned(value) => serde_json::Value::Number((*value).into()),
        DiagnosticValue::Number(value) => serde_json::Value::Number(
            serde_json::Number::from_f64(value.get()).expect("diagnostic numbers are finite"),
        ),
        DiagnosticValue::String(value) => serde_json::Value::String(value.clone()),
        DiagnosticValue::Array(values) => {
            serde_json::Value::Array(values.iter().map(diagnostic_value).collect())
        }
        DiagnosticValue::Object(values) => serde_json::Value::Object(
            values
                .iter()
                .map(|(key, value)| (key.clone(), diagnostic_value(value)))
                .collect(),
        ),
    }
}

fn scalar_value(value: &NormalizedScalar) -> serde_json::Value {
    match value {
        NormalizedScalar::String(value) | NormalizedScalar::Enum(value) => {
            serde_json::Value::String(value.clone())
        }
        NormalizedScalar::Integer(value) => serde_json::Value::Number((*value).into()),
        NormalizedScalar::Number(value) => serde_json::Value::Number(
            serde_json::Number::from_f64(value.get()).expect("normalized numbers are finite"),
        ),
        NormalizedScalar::Boolean(value) => serde_json::Value::Bool(*value),
    }
}

fn scalar_key(value: &NormalizedScalar) -> Vec<u8> {
    serde_json::to_vec(&scalar_value(value)).expect("normalized scalar is serializable")
}

fn valid_schema_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes.next().is_some_and(|byte| byte.is_ascii_lowercase())
        && bytes.all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' || byte == b'-'
        })
}

fn initial_project_toml(name: &str) -> String {
    format!(
        "format_version = 1\n[project]\nname = {name:?}\nschema = \".mara/schema.yaml\"\n[content]\ninclude = [\"**/*.mara.md\"]\nexclude = []\nrespect_gitignore = true\nfollow_directory_symlinks = false\nallow_internal_file_symlinks = true\n[index]\npath = \".mara/index.json\"\n[validation]\nwarnings_as_errors = false\n[git]\nrequire_clean_worktree_for_writes = true\n"
    )
}

fn initial_schema_yaml(name: &str) -> String {
    format!(
        "format_version: 1\nschema:\n  name: {name}\n  version: 0.1.0\nidentity:\n  mid:\n    format: ulid\n    prefix: m_\nflavours: {{}}\nrelations: {{}}\nrules: []\n"
    )
}

fn write_new(path: &Path, contents: &str) -> Result<(), OperationalErrorWire> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| io_failure(error, "could not create a Mara project file"))?;
    file.write_all(contents.as_bytes())
        .map_err(|error| io_failure(error, "could not write a Mara project file"))
}

fn io_failure(_error: io::Error, message: &'static str) -> OperationalErrorWire {
    OperationalErrorWire::new("io.failed", message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_io_error_envelope_uses_the_project_relative_affected_path() {
        let fixture = tempfile::tempdir().unwrap();
        initialize_project(fixture.path(), "index-error").unwrap();
        let project = discover_and_load(fixture.path()).unwrap();
        let error = IndexError::Io {
            operation: "write temporary index",
            path: project.index_path.clone(),
            source: io::Error::new(io::ErrorKind::PermissionDenied, "fixture"),
        };

        let wire = operational_index_error(&error, &project);
        let value = serde_json::to_value(wire).unwrap();

        assert_eq!(value["code"], "io.failed");
        assert_eq!(value["details"]["path"], ".mara/index.json");
        assert!(!value.to_string().contains(fixture.path().to_str().unwrap()));

        let error = IndexError::UnsafePath {
            reason: "fixture unsafe path",
            path: project.index_path.clone(),
        };
        let value = serde_json::to_value(operational_index_error(&error, &project)).unwrap();
        assert_eq!(value["code"], "io.failed");
        assert_eq!(value["details"]["path"], ".mara/index.json");
    }
}
