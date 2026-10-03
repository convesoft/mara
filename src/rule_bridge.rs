//! Bind shared rule evaluation on native corpus/code inputs to the source engine.
use crate::{
    CheckOutcome, Corpus, Diagnostic, Project, RuleChecks, Schema, SourceEngine, ValidationResult,
    ValidationSummary, ValidationTarget,
};

/// The native host supplies the corpus and its loading diagnostics when evaluating items.
/// A schema-only request supplies no corpus and performs definition checks only.
pub fn load_rule_checks(
    project: &Project,
    schema: &Schema,
    engine: &SourceEngine,
    target: ValidationTarget,
    corpus: Option<(&Corpus, &[Diagnostic])>,
) -> Result<RuleChecks, String> {
    if let Some((corpus, _)) = corpus
        && corpus.source_fingerprint != engine.source_fingerprint()
    {
        return Err("native corpus and rule-check snapshot differ".into());
    }
    let project_source = engine
        .files()
        .get(crate::PROJECT_FILE)
        .ok_or("missing explicit project source")?;
    let (supplied_project, _, _) = mara_engine::parse_project_for_validation(
        project.root().to_owned(),
        project_source,
        |path| {
            path.strip_prefix(project.root())
                .ok()
                .and_then(|path| source_key(path).ok())
                .is_some_and(|path| engine.files().contains_key(&path))
        },
    )
    .map_err(|e| e.to_string())?
    .into_parts();
    if supplied_project != *project {
        return Err("native project and explicit rule-check configuration differ".into());
    }
    let schema_path = project
        .schema_path()
        .strip_prefix(project.root())
        .map_err(|e| e.to_string())?;
    let source = engine
        .files()
        .get(&source_key(schema_path)?)
        .ok_or("missing explicit schema source")?;
    if mara_engine::parse_schema_for_validation(source)
        .map_err(|e| e.to_string())?
        .0
        != *schema
    {
        return Err("native schema and explicit rule-check schema differ".into());
    }
    let rules = crate::native_rules::load(project, schema);
    use sha2::{Digest, Sha256};
    for (path, fingerprint) in &rules.source_fingerprints {
        let supplied = engine.files().get(&source_key(path)?);
        if supplied
            .is_none_or(|source| format!("{:x}", Sha256::digest(source.as_bytes())) != *fingerprint)
        {
            return Err("native rule sources and explicit snapshot differ".into());
        }
    }
    Ok(checks_for_loaded(
        project,
        schema,
        &rules,
        target,
        corpus,
        engine.source_fingerprint(),
    ))
}

fn source_key(path: &std::path::Path) -> Result<String, String> {
    if let Ok(key) = mara_engine::normalize_source_path(&path.to_string_lossy()) {
        return Ok(key);
    }
    if !crate::is_project_relative(path) {
        return Err("unsafe native rule-check source path".into());
    }
    path.components()
        .filter(|part| !matches!(part, std::path::Component::CurDir))
        .collect::<std::path::PathBuf>()
        .to_str()
        .filter(|key| !key.is_empty() && !key.contains('\0'))
        .map(str::to_owned)
        .ok_or_else(|| "native rule-check source must name a UTF-8 file".into())
}

pub(crate) fn checks_for_loaded(
    project: &Project,
    schema: &Schema,
    rules: &crate::native_rules::Rules,
    target: ValidationTarget,
    corpus: Option<(&Corpus, &[Diagnostic])>,
    source_fingerprint: &str,
) -> RuleChecks {
    let definitions = CheckOutcome {
        evaluation_complete: rules.diagnostics.is_empty(),
        diagnostics: rules.diagnostics.clone(),
    };
    let evaluation = corpus.map(|(corpus, loading_diagnostics)| {
        let mut prerequisites = loading_diagnostics.to_vec();
        prerequisites.extend(crate::validate_corpus(corpus, schema));
        let mut result = ValidationResult {
            format_version: 1,
            project: project.root().to_owned(),
            target: target.clone(),
            valid: false,
            evaluation_complete: true,
            diagnostics: Vec::new(),
            summary: ValidationSummary {
                errors: 0,
                warnings: 0,
                counts_exact: true,
            },
            selection: None,
            has_more: false,
            next_cursor: None,
            path: None,
            flavours: None,
            relations: None,
        };
        if rules.diagnostics.is_empty() {
            rules.evaluate(corpus, schema, &prerequisites, &mut result);
        } else {
            result.evaluation_complete = false;
        }
        (
            target,
            CheckOutcome {
                evaluation_complete: result.evaluation_complete,
                diagnostics: result.diagnostics,
            },
        )
    });
    RuleChecks {
        source_fingerprint: source_fingerprint.to_owned(),
        definitions,
        evaluation,
    }
}
