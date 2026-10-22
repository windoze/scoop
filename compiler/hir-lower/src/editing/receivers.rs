use super::*;
use hir::{EditingUnavailable as U, EditingValue as V};

mod prefix;

#[derive(Default)]
pub(crate) struct RecoveredFacts {
    pub(super) receivers: Vec<hir::EditingReceiver>,
    pub(super) calls: Vec<hir::EditingIncompleteCall>,
}

#[derive(Clone)]
pub(crate) struct ReceiverObservation {
    file: usize,
    span: ast::Span,
    local: Option<hir::EditingLocalReference>,
}

impl DeclarationAnalysis {
    pub fn receivers(
        &mut self,
        function: hir::FunctionId,
        is_cancelled: impl Fn() -> bool,
    ) -> Result<&[hir::EditingReceiver], EditingCancelled> {
        self.ensure_recovered_facts(function, is_cancelled)?;
        Ok(&self.recovered_facts[&function].receivers)
    }

    pub(super) fn ensure_recovered_facts(
        &mut self,
        function: hir::FunctionId,
        is_cancelled: impl Fn() -> bool,
    ) -> Result<(), EditingCancelled> {
        check_cancelled(&is_cancelled)?;
        if !self.recovered_facts.contains_key(&function) {
            let facts = if let Some(body) = self.recovered_bodies.get(&function) {
                match self.signature(function) {
                    V::Unavailable(reason) => unavailable_facts(body, reason),
                    V::Known(_) => {
                        let mut candidate = self.lowerer.clone();
                        let facts =
                            candidate.check_recovered_facts(function, body, &is_cancelled)?;
                        check_cancelled(&is_cancelled)?;
                        self.lowerer = candidate;
                        facts
                    }
                }
            } else {
                RecoveredFacts::default()
            };
            self.recovered_facts.insert(function, facts);
        }
        Ok(())
    }
}

pub(super) fn check_cancelled(is_cancelled: &impl Fn() -> bool) -> Result<(), EditingCancelled> {
    if is_cancelled() {
        Err(EditingCancelled)
    } else {
        Ok(())
    }
}

fn unavailable_facts(body: &ast::RecoveredFunctionBody, reason: U) -> RecoveredFacts {
    let receivers = body
        .selectors
        .iter()
        .map(|selector| hir::EditingReceiver {
            span: match &selector.receiver {
                ast::RecoveredReceiver::Complete(receiver) => receiver.span(),
                ast::RecoveredReceiver::Unavailable(span) => *span,
            },
            dot: selector.dot,
            selector: selector.selector,
            safe_navigation: selector.navigation == ast::Navigation::Safe,
            value_type: V::Unavailable(reason),
            local: None,
            members: V::Unavailable(reason),
        })
        .collect();
    RecoveredFacts {
        receivers,
        calls: body
            .calls
            .iter()
            .map(|call| super::candidates::unavailable_call(call, reason))
            .collect(),
    }
}

impl Lowerer {
    pub(crate) fn observe_receiver_local(&mut self, local: hir::LocalId, span: ast::Span) {
        let Some(observation) = &self.editing_receiver else {
            return;
        };
        if observation.file != self.current_file || observation.span != span {
            return;
        }
        let local = &self.locals[local];
        let hir::LocalValueDefinitionSite::Source(definition) = local.definition else {
            return;
        };
        let reference = hir::EditingLocalReference {
            binding: local.binding,
            name: local.name.clone(),
            definition: hir::SemanticSourceLocation {
                source: self.source_identity(definition.file as usize).clone(),
                span: definition.span,
            },
        };
        self.editing_receiver
            .as_mut()
            .expect("active observation")
            .local = Some(reference);
    }

    fn check_receiver(
        &mut self,
        expression: &ast::Expr,
        facts: &mut hir::EditingReceiver,
        is_cancelled: &impl Fn() -> bool,
    ) -> Result<(), EditingCancelled> {
        let before = self.diagnostics.len();
        self.editing_receiver = Some(ReceiverObservation {
            file: self.current_file,
            span: expression.span(),
            local: None,
        });
        let value = self.lower_expr(expression, &mut Vec::new(), None);
        let observation = self.editing_receiver.take().expect("active observation");
        if let Some(value) = value.filter(|_| self.diagnostics.len() == before) {
            facts.value_type = V::Known(value.ty);
            facts.local = observation.local;
            facts.members =
                self.editing_receiver_members(&value, facts.safe_navigation, is_cancelled)?;
        } else {
            facts.value_type = V::Unavailable(U::BodyFailure);
            facts.members = V::Unavailable(U::BodyFailure);
        }
        Ok(())
    }
}
