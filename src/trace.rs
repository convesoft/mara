//! Native loading and fingerprinting for the shared trace computation.
use crate::{Corpus, Project, Schema, ValidationDiagnostic, ValidationError};
use mara_engine::trace::TraceInputs;
pub use mara_engine::trace::{
    TraceCheck, TraceField, TraceMatrixParams, TraceMatrixResult, TraceMatrixSummary,
    TraceSelection,
};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs};

pub(crate) fn matrix(
    project: &Project,
    schema: &Schema,
    params: &TraceMatrixParams,
) -> Result<TraceMatrixResult, ValidationError> {
    let (corpus, mut prerequisites) = crate::load_corpus_for_validation(project, schema)
        .map_err(|e| ValidationError::new("io_error", e.to_string()))?;
    prerequisites.extend(crate::validate_corpus(&corpus, schema));
    let files = params
        .check
        .as_ref()
        .map_or_else(|| project.rule_files.clone(), |c| c.files.clone());
    let sources = crate::native_rules::sources(project, files);
    let identity = snapshot_id(project, schema, &corpus, &sources, &prerequisites);
    mara_engine::trace::matrix_with_rule_loader(
        project,
        schema,
        params,
        TraceInputs {
            corpus: &corpus,
            prerequisites: &prerequisites,
            sources,
            source_identity: &identity,
            code_unavailable: false,
        },
        |schema, sources, bindings| {
            crate::native_rules::compile(schema, sources, bindings)
                .as_ref()
                .clone()
        },
    )
}

fn snapshot_id(
    project: &Project,
    schema: &Schema,
    corpus: &Corpus,
    sources: &[crate::native_rules::RuleSource],
    prerequisites: &[crate::Diagnostic],
) -> String {
    let mut hash = Sha256::new();
    hash.update(serde_json::to_vec(schema).expect("schema serializes"));
    for path in [
        project.root().join(crate::PROJECT_FILE),
        project.schema_path().to_owned(),
    ]
    .into_iter()
    .chain(
        sources
            .iter()
            .filter(|s| s.content.is_ok())
            .map(|s| project.root().join(&s.path)),
    ) {
        hash.update(
            path.strip_prefix(project.root())
                .unwrap_or(&path)
                .as_os_str()
                .as_encoded_bytes(),
        );
        if let Ok(bytes) = fs::read(&path) {
            hash.update(bytes)
        }
    }
    for document in corpus.documents() {
        hash.update(document.path().as_os_str().as_encoded_bytes());
        hash.update(document.source().as_bytes());
    }
    for file in corpus.code().files() {
        hash.update(b"code-file");
        hash.update(file.path.as_os_str().as_encoded_bytes());
        hash.update(file.source.as_bytes());
    }
    for path in corpus.code().assets() {
        hash.update(b"code-asset");
        hash.update(path.as_os_str().as_encoded_bytes());
        match fs::read(project.root().join(path)) {
            Ok(bytes) => {
                hash.update([1]);
                hash.update(bytes);
            }
            Err(_) => hash.update([0]),
        }
    }
    for path in corpus.file_only_code_paths() {
        hash.update(b"file-only-code");
        hash.update(path.as_os_str().as_encoded_bytes());
        match corpus.code().file_only_bytes(&path) {
            Some(bytes) => {
                hash.update([1]);
                hash.update(bytes);
            }
            None => hash.update([0]),
        }
    }
    let retained = corpus
        .documents()
        .iter()
        .map(|document| document.path())
        .collect::<BTreeSet<_>>();
    let mut excluded = BTreeSet::new();
    for diagnostic in prerequisites {
        hash.update(
            serde_json::to_vec(&ValidationDiagnostic::from_source(diagnostic))
                .expect("diagnostic serializes"),
        );
        let path = diagnostic.source().path();
        if !retained.contains(path) {
            excluded.insert(path);
        }
    }
    for path in excluded {
        hash.update(b"excluded-source");
        hash.update(path.as_os_str().as_encoded_bytes());
        if let Ok(bytes) = fs::read(project.root().join(path)) {
            hash.update(bytes);
        }
    }
    format!("{:x}", hash.finalize())
}
