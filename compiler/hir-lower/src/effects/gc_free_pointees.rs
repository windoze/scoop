//! Symbolic GC-free pointee requirements for generic declarations.

use scoop_ast::Span;
use scoop_hir as hir;

mod call_sites;
mod inference;
mod occurrences;
mod requirements;
mod validation;

#[derive(Debug, Clone, Copy)]
struct PointeeApplicationOccurrence {
    ty: hir::TypeId,
    file: usize,
    span: Span,
    context: RequirementContext,
}

#[derive(Debug, Clone)]
struct PointeeRequirementCallSite {
    context: RequirementContext,
    callee: hir::FunctionId,
    arguments: Vec<(hir::TypeParamId, hir::TypeId)>,
    file: usize,
    span: Span,
}

impl PointeeRequirementCallSite {
    fn argument(&self, parameter: hir::TypeParamId) -> hir::TypeId {
        self.arguments
            .iter()
            .find_map(|(candidate, argument)| (*candidate == parameter).then_some(*argument))
            .expect("every pointee predicate edge binds every callable parameter")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RequirementContext {
    Function(hir::FunctionId),
    Nominal(crate::Owner),
    Closed,
}
