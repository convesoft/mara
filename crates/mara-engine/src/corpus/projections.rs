//! Reusable owned projections. Source identity, not cache lifetime, owns handles.
use super::*;
use crate::{discovery::DiscoveryProjection, relations::RelationGraph};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Default)]
pub struct ProjectionCache(Arc<Mutex<State>>);
#[derive(Debug, Default)]
struct State {
    discovery: Option<(DocumentSet, Arc<DiscoveryProjection>)>,
    relations: Option<Relations>,
    discovery_builds: usize,
    relation_builds: usize,
}
#[derive(Debug)]
struct Relations {
    documents: DocumentSet,
    code: crate::CodeIndex,
    schema: Schema,
    file_only: Vec<(PathBuf, Option<Vec<u8>>)>,
    graph: Arc<RelationGraph>,
}
// Disposable state does not participate in corpus semantic equality.
impl PartialEq for ProjectionCache {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
impl Eq for ProjectionCache {}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProjectionActivity {
    pub discovery_builds: usize,
    pub relation_builds: usize,
}
impl Corpus {
    pub fn projection_activity(&self) -> ProjectionActivity {
        let s = self.projections.0.lock().unwrap();
        ProjectionActivity {
            discovery_builds: s.discovery_builds,
            relation_builds: s.relation_builds,
        }
    }
    pub(crate) fn discovery_projection(&self) -> Arc<DiscoveryProjection> {
        let mut s = self.projections.0.lock().unwrap();
        if let Some((documents, graph)) = &s.discovery
            && *documents == self.documents
        {
            return graph.clone();
        }
        let graph = Arc::new(crate::DiscoveryGraph::project(self));
        s.discovery = Some((self.documents.clone(), graph.clone()));
        s.discovery_builds += 1;
        graph
    }
    pub fn relation_graph(&self, schema: &Schema) -> Arc<RelationGraph> {
        let file_only = self
            .file_only_code_paths()
            .into_iter()
            .map(|p| {
                let bytes = self.code.file_only_bytes(&p);
                (p, bytes)
            })
            .collect::<Vec<_>>();
        let mut s = self.projections.0.lock().unwrap();
        if let Some(r) = &s.relations
            && r.documents == self.documents
            && r.code == self.code
            && r.schema == *schema
            && r.file_only == file_only
        {
            return r.graph.clone();
        }
        let graph = Arc::new(RelationGraph::new(self, schema));
        s.relations = Some(Relations {
            documents: self.documents.clone(),
            code: self.code.clone(),
            schema: schema.clone(),
            file_only,
            graph: graph.clone(),
        });
        s.relation_builds += 1;
        graph
    }
}
