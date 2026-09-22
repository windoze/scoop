//! Candidate-local probing and winner-only commit for ordinary dependencies.

use hir::ImportedCallableSource;
use scoop_ast as ast;
use scoop_hir as hir;

use super::*;
use crate::call_resolution::specificity::DeclarationForwardingView;

mod arguments;
mod candidate;
mod commit;
mod defaults;
mod inputs;
mod pattern;
mod probe;

pub(in crate::expr) use arguments::ImportedArgumentMap;
use candidate::ImportedCallableCandidate;
use defaults::ImportedDefaultPlan;
use inputs::ImportedCallReceiver;
pub(in crate::expr) use inputs::{
    ImportedCallArguments, ImportedMemberReceiver, ImportedProbeCall,
};

pub(crate) struct ImportedDependencyCallProbe {
    state: Box<Lowerer>,
    candidate: ImportedCallableCandidate,
    receiver: ImportedCallReceiver,
    source_args: Vec<hir::Expr>,
    argument_sinks: Vec<Vec<hir::Statement>>,
    argument_map: ImportedArgumentMap,
    default_plan: ImportedDefaultPlan,
    parameter_types: Vec<hir::TypeId>,
    forwarding_parameters: Vec<hir::TypeId>,
    result_type: hir::TypeId,
    integer_arguments: Vec<Option<hir::IntegerKind>>,
    declaration_file: usize,
    declaration_span: ast::Span,
    call_span: ast::Span,
}

impl ImportedDependencyCallProbe {
    pub(crate) fn forwarding(&self) -> DeclarationForwardingView<'_> {
        DeclarationForwardingView::nominal_parameters(&[], &self.forwarding_parameters)
    }

    pub(crate) fn parameterized(&self) -> bool {
        !self.candidate.interface().type_parameters().is_empty()
    }

    pub(crate) fn defaults(&self) -> usize {
        self.argument_map.defaults()
    }

    pub(crate) fn vararg(&self) -> bool {
        self.argument_map.has_vararg()
    }

    pub(crate) fn source_argument_integer(&self, index: usize) -> Option<hir::IntegerKind> {
        self.integer_arguments[index]
    }

    pub(crate) fn signature(&self, state: &Lowerer, name: &str) -> String {
        let parameters = self
            .candidate
            .source_interface()
            .expect("callable candidates retain their validated source interface")
            .parameters()
            .parameters()
            .iter()
            .zip(&self.parameter_types)
            .map(|(parameter, ty)| {
                format!("{}: {}", parameter.name().as_str(), state.type_name(*ty))
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "{name}({parameters}): {}",
            state.type_name(self.result_type)
        )
    }

    pub(crate) const fn declaration_location(&self) -> (usize, ast::Span) {
        (self.declaration_file, self.declaration_span)
    }
}
