//! Candidate-local probing and winner-only commit for ordinary dependencies.

use hir::ImportedCallableSource;
use scoop_ast as ast;
use scoop_hir as hir;

use super::*;
use crate::call_resolution::specificity::{DeclarationForwardingView, OwnedDeclarationForwarding};

mod arguments;
mod candidate;
mod commit;
mod defaults;
mod generic;
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

enum ImportedCallImplementation {
    Native,
    Generic {
        template: hir::ImportedGenericCallableTemplateId,
        arguments: hir::NonEmptyVec<hir::TypeId>,
    },
}

pub(crate) struct ImportedDependencyCallProbe {
    implementation: ImportedCallImplementation,
    state: Box<Lowerer>,
    candidate: ImportedCallableCandidate,
    receiver: ImportedCallReceiver,
    source_args: Vec<hir::Expr>,
    argument_sinks: Vec<Vec<hir::Statement>>,
    argument_map: ImportedArgumentMap,
    default_plan: ImportedDefaultPlan,
    parameter_types: Vec<hir::TypeId>,
    result_type: hir::TypeId,
    integer_arguments: Vec<Option<hir::IntegerKind>>,
    declaration_file: usize,
    declaration_span: ast::Span,
    call_span: ast::Span,
}

impl ImportedDependencyCallProbe {
    pub(crate) fn forwarding(&self, state: &mut Lowerer) -> OwnedDeclarationForwarding {
        let (parameters, bindings) = match self.implementation {
            ImportedCallImplementation::Native => (Vec::new(), Default::default()),
            ImportedCallImplementation::Generic { template, .. } => {
                let declaration = self.state.imported_generic_templates[template]
                    .declaration
                    .clone();
                let template = state
                    .request_imported_generic_template(declaration)
                    .expect("an applicable imported candidate has a resolved declaration");
                let prepared = &state.imported_generic_templates[template];
                (prepared.type_parameters.clone(), prepared.bindings.clone())
            }
        };
        let parameter_types = self
            .candidate
            .interface()
            .receiver()
            .into_iter()
            .chain(self.argument_map.source_parameters())
            .map(|signature| {
                state
                    .imported_signature_type_with_bindings(signature, &bindings)
                    .expect("an applicable imported candidate has resolved input types")
            })
            .collect::<Vec<_>>();
        DeclarationForwardingView::callable_parameters(&parameters, &parameter_types).to_owned()
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

    pub(crate) fn signature(&self, name: &str) -> String {
        let state = &self.state;
        if let ImportedCallImplementation::Generic { template, .. } = self.implementation {
            let signature = &state.imported_generic_templates[template].signature;
            let binders = &signature.type_parameters;
            let type_parameters = crate::call_resolution::diagnostics::render_type_parameters(
                state, binders, binders,
            );
            let receiver = signature
                .receiver
                .map(|ty| format!("{}.", state.type_name_with_params(ty, binders)))
                .unwrap_or_default();
            let parameters = signature
                .parameters
                .iter()
                .skip(usize::from(signature.receiver.is_some()))
                .map(|parameter| {
                    format!(
                        "{}: {}",
                        parameter.name,
                        state.type_name_with_params(parameter.ty, binders)
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            return format!(
                "{receiver}{name}{type_parameters}({parameters}): {}",
                state.type_name_with_params(signature.return_type, binders)
            );
        }
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
