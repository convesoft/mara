//! Execution-only native code inputs; never serialized into document snapshots.
use super::*;
use crate::{CodeIndex, Corpus};
use std::{
    fmt,
    sync::{Arc, OnceLock},
};

/// A trusted native host supplies the existing code projection and content identity.
/// An unavailable reason preserves partial facts without certifying complete coverage.
#[derive(Debug, Clone)]
pub struct NativeCodeInputs {
    pub index: CodeIndex,
    pub identity: String,
    pub unavailable_reason: Option<String>,
}

pub(crate) struct NativeCodeLoader {
    load: Box<dyn Fn() -> NativeCodeInputs + Send + Sync>,
    inputs: OnceLock<NativeCodeInputs>,
}
impl fmt::Debug for NativeCodeLoader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NativeCodeLoader").finish_non_exhaustive()
    }
}
impl NativeCodeLoader {
    pub(super) fn inputs(&self) -> &NativeCodeInputs {
        self.inputs.get_or_init(|| (self.load)())
    }
}

pub(super) fn code_unavailable(loader: Option<&NativeCodeLoader>, id: &str) -> Option<String> {
    match loader {
        None => Some(format!(
            "required code facts for named definition {id} were not supplied to this snapshot"
        )),
        Some(loader) => loader.inputs().unavailable_reason.as_ref().map(|reason| {
            format!("required code facts for named definition {id} are incomplete: {reason}")
        }),
    }
}

impl SourceEngine {
    /// Native hosts retain acquisition outside the portable engine. Only trusted native
    /// construction can install this hook; ordinary workspace snapshots cannot import it.
    pub fn with_native_query_code(
        mut self,
        load: impl Fn() -> NativeCodeInputs + Send + Sync + 'static,
    ) -> Result<Self, crate::Error> {
        if !self.native_paths {
            return Err(crate::Error::InvalidProject {
                path: crate::PROJECT_FILE.into(),
                message: "native query code inputs require trusted native construction".into(),
            });
        }
        self.native_query_code = Some(Arc::new(NativeCodeLoader {
            load: Box::new(load),
            inputs: OnceLock::new(),
        }));
        Ok(self)
    }

    pub(crate) fn query_fingerprint(&self) -> String {
        match self
            .native_query_code
            .as_ref()
            .and_then(|loader| loader.inputs.get())
        {
            Some(inputs) => hash(&(
                "native-query-input-1",
                self.source_fingerprint(),
                &inputs.identity,
            )),
            None => self.source_fingerprint().into(),
        }
    }

    pub(super) fn query_corpus(
        &self,
    ) -> Result<(crate::Project, crate::Schema, Corpus), crate::Error> {
        let (project, schema, mut corpus) = self.corpus()?;
        if let Some(inputs) = self
            .native_query_code
            .as_ref()
            .and_then(|loader| loader.inputs.get())
        {
            corpus.code = inputs.index.clone();
            corpus.source_fingerprint = self.query_fingerprint();
        }
        Ok((project, schema, corpus))
    }
}
