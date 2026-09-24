//! A public-API adapter around the unmodified native SHACL validators.
//!
//! Keep an error ledger across nested calls: some upstream logical validators
//! turn errors into nonconformance. The caller must inspect `failed` even when
//! the outer shape returns Ok. Constraint semantics remain upstream-owned.
use std::{cell::Cell, collections::BTreeMap};

use rudof_iri::IriS;
use rudof_rdf::{
    rdf_core::{Rdf, SHACLPath, term::Object},
    rdf_impl::OxigraphInMemory,
};
use shacl::{
    error::ValidationError,
    ir::{IRComponent, IRPropertyShape, IRSchema, IRShape, ShapeLabelIdx},
    validator::{
        RecursionSemantics,
        constraints::NativeValidator,
        engine::{Engine, NativeEngine},
        nodes::{FocusNodes, ValueNodes},
        report::ValidationOutcome,
    },
};

type Graph = OxigraphInMemory;
type Node = <Graph as Rdf>::Term;

pub(super) struct RuleEngine {
    native: Box<dyn Engine<Graph>>,
    pub failed: Cell<bool>,
    pub counts: BTreeMap<(String, String, String), (usize, Option<usize>)>,
}

impl RuleEngine {
    pub fn new() -> Self {
        Self {
            native: Box::new(NativeEngine::new(RecursionSemantics::default())),
            failed: Cell::new(false),
            counts: BTreeMap::new(),
        }
    }
}

impl Engine<Graph> for RuleEngine {
    fn fork(&self) -> Box<dyn Engine<Graph>> {
        // Mara invokes one focus node synchronously and never uses the parallel
        // processor. A fork starts an independent evaluation and error ledger.
        Box::new(Self::new())
    }

    fn evaluate(
        &mut self,
        store: &Graph,
        shape: &IRShape,
        component: &IRComponent,
        values: &ValueNodes<Graph>,
        source: Option<&IRShape>,
        path: Option<&SHACLPath>,
        schema: &IRSchema,
    ) -> Result<ValidationOutcome, ValidationError> {
        let validator: &dyn NativeValidator<Graph> = match component {
            IRComponent::Class(c) => c,
            IRComponent::Datatype(c) => c,
            IRComponent::MinCount(c) => c,
            IRComponent::MaxCount(c) => c,
            IRComponent::Pattern(c) => c,
            IRComponent::Or(c) => c,
            IRComponent::And(c) => c,
            IRComponent::Not(c) => c,
            IRComponent::Node(c) => c,
            IRComponent::HasValue(c) => c,
            IRComponent::In(c) => c,
            IRComponent::QualifiedValueShape(c) => c,
            _ => {
                self.failed.set(true);
                return Err(ValidationError::UnsupportedMode(
                    "constraint outside Mara's validated profile".into(),
                ));
            }
        };
        let outcome =
            validator.validate_native(component, shape, store, self, values, source, path, schema);
        if outcome.is_err() {
            self.failed.set(true);
        }
        // Report counts from the native evaluator's completed shape outcomes.
        // Missing outcomes stay unknown; never infer nonconformance from absence.
        if outcome.is_ok() && !self.failed.get() {
            for (focus, nodes) in values.iter() {
                let Ok(focus) = Graph::term_as_object(focus) else {
                    continue;
                };
                let qualifying = match component {
                    IRComponent::QualifiedValueShape(c) => nodes
                        .iter()
                        .map(|node| {
                            let object = Graph::term_as_object(node).ok()?;
                            self.native
                                .get_cached_outcome(&object, *c.shape())
                                .map(|result| usize::from(result.conforms()))
                        })
                        .sum::<Option<usize>>(),
                    IRComponent::MinCount(_) | IRComponent::MaxCount(_) => Some(nodes.len()),
                    _ => continue,
                };
                let components: &[&str] = match component {
                    IRComponent::QualifiedValueShape(_) => {
                        &["QualifiedMinCount", "QualifiedMaxCount"]
                    }
                    IRComponent::MinCount(_) => &["MinCount"],
                    IRComponent::MaxCount(_) => &["MaxCount"],
                    _ => unreachable!(),
                };
                for name in components {
                    self.counts.insert(
                        (
                            shape.id().to_string(),
                            focus.to_string(),
                            format!("{}{}ConstraintComponent", super::SH, name),
                        ),
                        (nodes.len(), qualifying),
                    );
                }
            }
        }
        outcome
    }

    fn path(
        &self,
        store: &Graph,
        shape: &IRPropertyShape,
        focus: &Node,
    ) -> Result<FocusNodes<Graph>, ValidationError> {
        let result = self.native.path(store, shape, focus);
        if result.is_err() {
            self.failed.set(true);
        }
        result
    }

    fn target_node(
        &self,
        store: &Graph,
        node: &Object,
    ) -> Result<FocusNodes<Graph>, ValidationError> {
        self.native.target_node(store, node)
    }
    fn target_class(
        &self,
        store: &Graph,
        class: &Object,
    ) -> Result<FocusNodes<Graph>, ValidationError> {
        self.native.target_class(store, class)
    }
    fn target_subject_of(
        &self,
        store: &Graph,
        pred: &IriS,
    ) -> Result<FocusNodes<Graph>, ValidationError> {
        self.native.target_subject_of(store, pred)
    }
    fn target_object_of(
        &self,
        store: &Graph,
        pred: &IriS,
    ) -> Result<FocusNodes<Graph>, ValidationError> {
        self.native.target_object_of(store, pred)
    }
    fn implicit_target_class(
        &self,
        store: &Graph,
        shape: &Object,
    ) -> Result<FocusNodes<Graph>, ValidationError> {
        self.native.implicit_target_class(store, shape)
    }
    fn record_validation(
        &mut self,
        node: Object,
        shape: ShapeLabelIdx,
        outcome: ValidationOutcome,
    ) {
        self.native.record_validation(node, shape, outcome);
    }
    fn has_validated(&self, node: &Object, shape: ShapeLabelIdx) -> bool {
        self.native.has_validated(node, shape)
    }
    fn get_cached_outcome(&self, node: &Object, shape: ShapeLabelIdx) -> Option<ValidationOutcome> {
        self.native.get_cached_outcome(node, shape)
    }
    fn recursion_semantics(&self) -> RecursionSemantics {
        self.native.recursion_semantics()
    }
    fn is_in_chain(&self, node: &Object, shape: ShapeLabelIdx) -> bool {
        self.native.is_in_chain(node, shape)
    }
    fn chain_enter(&mut self, node: Object, shape: ShapeLabelIdx) {
        self.native.chain_enter(node, shape);
    }
    fn chain_exit(&mut self, node: &Object, shape: ShapeLabelIdx) {
        self.native.chain_exit(node, shape);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rudof_rdf::{rdf_core::RDFFormat, rdf_impl::ReaderMode};
    use shacl::validator::engine::Validate;

    #[test]
    fn ledger_preserves_errors_swallowed_by_native_not_and_qualification() {
        // Exercise native nested validation, bypassing Mara's definition checks
        // to provoke a real adapter error for an unsupported component. The
        // outer validators swallow that error into a passing result; the ledger
        // must prevent the caller from accepting it as a complete evaluation.
        for obligation in [
            "sh:not [ sh:minLength 1 ]",
            "sh:property [ sh:path <urn:test:p>; sh:qualifiedValueShape [ sh:minLength 1 ]; sh:qualifiedMaxCount 0 ]",
        ] {
            let shapes = IRSchema::from_str(
                &format!("@prefix sh: <http://www.w3.org/ns/shacl#> . <urn:test:root> a sh:NodeShape; {obligation} ."),
                &RDFFormat::Turtle,
                None,
                &ReaderMode::Strict,
            ).unwrap();
            let data = Graph::from_str(
                "<urn:test:item> <urn:test:p> <urn:test:value> .",
                &RDFFormat::Turtle,
                None,
                &ReaderMode::Strict,
            )
            .unwrap();
            let mut engine = RuleEngine::new();
            let shape = shapes
                .get_shape(&Object::iri(IriS::new("urn:test:root").unwrap()))
                .unwrap();
            let outcome = shape
                .validate(
                    &data,
                    &mut engine,
                    Some(&FocusNodes::single(
                        Object::iri(IriS::new("urn:test:item").unwrap()).into(),
                    )),
                    None,
                    &shapes,
                )
                .unwrap();
            assert!(
                outcome.conforms(),
                "upstream behavior changed for {obligation}"
            );
            assert!(
                engine.failed.get(),
                "nested error was lost for {obligation}"
            );
        }
    }
}
