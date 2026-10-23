use super::*;
use crate::Owner;
use hir::{EditingConstructionBody as Body, EditingUnavailable as U, EditingValue as V};
use std::rc::Rc;

mod inputs;
mod observation;
mod sources;
use inputs::ConstructionInput;

#[derive(Clone, Default)]
pub(crate) struct ConstructionInputs {
    sources: imbl::HashMap<Owner, Rc<ConstructionInput>>,
    properties: imbl::HashMap<hir::PropertyId, (usize, ast::Span)>,
    bases: imbl::HashMap<hir::ClassId, ast::SupertypeSpec>,
    checked: imbl::HashSet<Owner>,
    failed: imbl::HashSet<Owner>,
}

impl DeclarationAnalysis {
    pub(super) fn analyze_construction_body(
        &mut self,
        body: Body,
        is_cancelled: impl Fn() -> bool + 'static,
    ) -> Result<V<ObservedBody>, EditingCancelled> {
        if !self.declarations_ready {
            return Ok(V::Unavailable(U::DeclarationFailure));
        }
        let id = hir::EditingSourceBodyId::Construction(body);
        let Some(source) = self.lowerer.editing_construction_source(body) else {
            return Ok(V::Unavailable(U::IncompleteSyntax));
        };
        if !self.declaration_scope_available(&source.origin) {
            return Ok(V::Unavailable(U::BodyFailure));
        }
        let owner = self.lowerer.construction_body_owner(body);
        self.check_construction(owner, is_cancelled)?;
        Ok(self
            .lowerer
            .editing_prepared_bodies
            .as_mut()
            .and_then(|facts| facts.remove(&id))
            .map_or(V::Unavailable(U::BodyFailure), V::Known))
    }

    /// Include implicit delegation and declarations with no explicit source bodies.
    pub fn check_source_constructions(
        &mut self,
        source: Option<&scoop_identity::SourceIdentity>,
        is_cancelled: impl Fn() -> bool + 'static,
    ) -> Result<Vec<hir::SemanticSourceLocation>, EditingCancelled> {
        let check = Rc::new(is_cancelled);
        let mut inputs = self
            .lowerer
            .editing_constructions
            .as_ref()
            .into_iter()
            .flat_map(|inputs| inputs.sources.values())
            .filter(|input| {
                source.is_none_or(|source| self.lowerer.source_identity(input.file) == source)
            })
            .cloned()
            .collect::<Vec<_>>();
        inputs.sort_by_key(|input| (input.file, input.origin(&self.lowerer).span.start));
        let mut unavailable = Vec::new();
        for input in inputs {
            if check() {
                return Err(EditingCancelled);
            }
            let origin = input.origin(&self.lowerer);
            if !self.declaration_scope_available(&origin) {
                continue;
            }
            if !self.declarations_ready {
                unavailable.push(origin);
                continue;
            }
            let cancellation = check.clone();
            self.check_construction(input.owner(), move || cancellation())?;
            if self
                .lowerer
                .editing_constructions
                .as_ref()
                .expect("editing constructions")
                .failed
                .contains(&input.owner())
            {
                unavailable.push(origin);
            }
        }
        Ok(unavailable)
    }

    fn check_construction(
        &mut self,
        owner: Owner,
        is_cancelled: impl Fn() -> bool + 'static,
    ) -> Result<(), EditingCancelled> {
        if is_cancelled() {
            return Err(EditingCancelled);
        }
        let inputs = self
            .lowerer
            .editing_constructions
            .as_ref()
            .expect("editing constructions");
        if inputs.checked.contains(&owner) {
            return Ok(());
        }
        let input = inputs.sources[&owner].clone();
        let mut candidate = self.lowerer.clone();
        let cancellation = crate::pipeline::cancellation::FrontendCancellation::new(is_cancelled);
        let previous = candidate.cancellation.replace(cancellation.clone());
        let before = candidate.diagnostics.len();
        input.lower(&mut candidate);
        candidate.finish_editing_construction(owner, candidate.diagnostics.len() == before);
        if cancellation.cancelled() {
            return Err(EditingCancelled);
        }
        candidate.cancellation = previous;
        self.lowerer = candidate;
        Ok(())
    }
}

impl Lowerer {
    pub(crate) fn fail_editing_construction(&mut self, owner: Owner) {
        if let Some(inputs) = &mut self.editing_constructions {
            inputs.failed.insert(owner);
        }
    }

    pub(crate) fn finish_editing_construction(&mut self, owner: Owner, valid: bool) {
        if let Some(inputs) = &mut self.editing_constructions {
            inputs.checked.insert(owner);
            if !valid {
                inputs.failed.insert(owner);
            }
        }
    }
}
