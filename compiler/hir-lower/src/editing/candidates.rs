use super::*;
use crate::call_resolution::candidates::{ArgumentMode, DeclarationSignature};
use hir::{EditingUnavailable as U, EditingValue as V};

mod extensions;
mod imported;
pub(crate) mod inference;
mod lookup;
mod members;
mod properties;
mod source;
mod values;

pub(crate) type Candidate = Result<Option<hir::EditingCallCandidate>, U>;

impl DeclarationAnalysis {
    pub fn incomplete_calls(
        &mut self,
        function: hir::FunctionId,
        is_cancelled: impl Fn() -> bool,
    ) -> Result<&[hir::EditingIncompleteCall], EditingCancelled> {
        self.ensure_recovered_facts(function, is_cancelled)?;
        Ok(&self.recovered_facts[&function].calls)
    }
}

pub(super) fn unavailable_call(call: &ast::RecoveredCall, reason: U) -> hir::EditingIncompleteCall {
    let name = call_name(call).map(|name| name.text.clone());
    hir::EditingIncompleteCall {
        span: call.span,
        name,
        opening: call.arguments.opening,
        commas: call.arguments.commas.clone(),
        arguments_end: call.arguments.end,
        closed: call.arguments.closed,
        argument_spans: call
            .arguments
            .items
            .iter()
            .map(|argument| argument.span)
            .collect(),
        trailing: call.trailing.as_ref().map(|argument| argument.span),
        candidates: V::Unavailable(reason),
    }
}

fn call_name(call: &ast::RecoveredCall) -> Option<&ast::Ident> {
    use ast::RecoveredCallTarget as Target;
    match &call.target {
        Target::Named(name)
        | Target::Member { name, .. }
        | Target::Super { name, .. }
        | Target::QualifiedSuper { name, .. } => Some(name),
        Target::Invoke(_) | Target::Unavailable(_) => None,
    }
}

fn arguments(call: &ast::RecoveredCall) -> impl Iterator<Item = &ast::RecoveredCallArgument> {
    call.arguments.items.iter().chain(&call.trailing)
}

#[derive(Default)]
pub(crate) struct Candidates {
    pub(crate) values: Vec<hir::EditingCallCandidate>,
    pub(crate) unavailable: Option<U>,
}

impl Candidates {
    pub(crate) fn add(&mut self, candidate: Candidate) {
        match candidate {
            Ok(Some(candidate)) => self.values.push(candidate),
            Ok(None) => {}
            Err(reason) => {
                self.unavailable.get_or_insert(reason);
            }
        }
    }

    pub(crate) fn finish(self) -> V<Vec<hir::EditingCallCandidate>> {
        match self.unavailable {
            Some(reason) => V::Unavailable(reason),
            None => V::Known(self.values),
        }
    }
}
