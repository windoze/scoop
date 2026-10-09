//! Candidate-local probing and winner-only commit for ordinary dependencies.

use hir::ImportedCallableSource;
use scoop_ast as ast;
use scoop_hir as hir;

use super::*;
use crate::call_resolution::specificity::{DeclarationForwardingView, OwnedDeclarationForwarding};

mod arguments;
mod bound_calls;
mod candidate;
mod commit;
mod defaults;
mod generic;
mod inputs;
mod intrinsics;
mod pattern;
mod probe;
mod properties;
mod signature;

pub(in crate::expr) use signature::DependencySignature;

pub(in crate::expr) use arguments::ImportedArgumentMap;
pub(in crate::expr) use candidate::ImportedCallableCandidate;
use defaults::ImportedDefaultPlan;
pub(in crate::expr) use generic::ImportedGenericTarget;
use inputs::ImportedCallReceiver;
pub(in crate::expr) use inputs::{
    ImportedCallArguments, ImportedMemberReceiver, ImportedProbeCall,
};

enum ImportedCallImplementation {
    Native,
    Generic {
        template: generic::ImportedGenericTarget,
        arguments: hir::ImportedCallableArguments,
    },
    Intrinsic {
        template: generic::ImportedGenericTarget,
        operation: ImportedIntrinsicCall,
    },
}

enum ImportedIntrinsicCall {
    MaybeUninit(hir::MaybeUninitIntrinsic),
    Atomic(hir::AtomicIntrinsic),
    Expression(hir::Expr),
    PointerMember(hir::PointerIntrinsic),
    ArrayConversion(hir::ArrayIntrinsic),
    ArrayAccess(hir::ArrayAccessKind),
    ForeignCallback {
        kind: hir::IntrinsicFunctionKind,
        native_type: hir::TypeId,
    },
}

pub(crate) struct ImportedDependencyCallProbe {
    implementation: ImportedCallImplementation,
    state: Box<Lowerer>,
    candidate: ImportedCallableCandidate,
    signature: DependencySignature,
    declared_receiver: Option<hir::TypeId>,
    receiver: ImportedCallReceiver,
    source_args: Vec<hir::Expr>,
    argument_sinks: Vec<Vec<hir::Statement>>,
    argument_map: ImportedArgumentMap,
    default_plan: ImportedDefaultPlan,
    parameter_types: Vec<hir::TypeId>,
    result_type: hir::TypeId,
    numeric_arguments: Vec<Option<crate::call_resolution::specificity::NumericLiteralKind>>,
    declaration_file: usize,
    declaration_span: ast::Span,
    call_span: ast::Span,
}

impl ImportedDependencyCallProbe {
    pub(in crate::expr) fn safety(&self) -> hir::CallableSafetyV1 {
        self.candidate.interface().effects().safety()
    }

    pub(crate) fn forwarding(&self, state: &mut Lowerer) -> OwnedDeclarationForwarding {
        let (signature, receiver) = match &self.implementation {
            ImportedCallImplementation::Native => {
                let signature = state
                    .imported_native_signature(&self.candidate)
                    .expect("an applicable imported candidate has a resolved signature");
                let receiver = self.candidate.interface().receiver().map(|ty| {
                    state
                        .imported_signature_type(ty)
                        .expect("an applicable extension has a resolved receiver")
                });
                (signature, receiver)
            }
            ImportedCallImplementation::Generic { template, .. }
            | ImportedCallImplementation::Intrinsic { template, .. } => {
                let declaration = template.declaration(&self.state);
                let template = generic::ImportedGenericTarget::request(state, declaration)
                    .expect("an applicable imported candidate has a resolved declaration");
                let (loaded, _) = template.signature(state);
                let receiver = self.candidate.interface().receiver().and(loaded.receiver);
                (loaded.signature, receiver)
            }
        };
        let mut parameter_types = self
            .argument_map
            .mapping()
            .forwarding_parameter_types(&signature.value_parameters);
        if let Some(receiver) = receiver {
            parameter_types.insert(0, receiver);
        }
        DeclarationForwardingView::parameter_groups(
            &signature.owner_parameters,
            &signature.callable_parameters,
            &parameter_types,
        )
        .to_owned()
    }

    pub(crate) fn parameterized(&self) -> bool {
        matches!(
            self.implementation,
            ImportedCallImplementation::Generic {
                template: generic::ImportedGenericTarget::Constructor(_)
                    | generic::ImportedGenericTarget::Variant(_),
                ..
            }
        ) || !self.candidate.interface().type_parameters().is_empty()
    }

    pub(crate) fn defaults(&self) -> usize {
        self.argument_map.defaults()
    }

    pub(crate) fn vararg(&self) -> bool {
        self.argument_map.has_vararg()
    }

    pub(crate) fn source_argument_numeric(
        &self,
        index: usize,
    ) -> Option<crate::call_resolution::specificity::NumericLiteralKind> {
        self.numeric_arguments[index]
    }

    pub(crate) fn signature(&self, name: &str) -> String {
        let state = &self.state;
        if let ImportedCallImplementation::Generic { template, .. }
        | ImportedCallImplementation::Intrinsic { template, .. } = &self.implementation
        {
            let signature = &self.signature;
            let all_parameters = signature
                .owner_parameters
                .iter()
                .chain(&signature.callable_parameters)
                .cloned()
                .collect::<Vec<_>>();
            let binders = all_parameters.as_slice();
            let explicit_parameters = if matches!(
                template,
                generic::ImportedGenericTarget::Constructor(_)
                    | generic::ImportedGenericTarget::Variant(_)
            ) {
                &signature.owner_parameters
            } else {
                &signature.callable_parameters
            };
            let type_parameters = crate::call_resolution::diagnostics::render_type_parameters(
                state,
                explicit_parameters,
                binders,
            );
            let receiver = self
                .declared_receiver
                .map(|ty| format!("{}.", state.type_name_with_params(ty, binders)))
                .unwrap_or_default();
            let parameters = signature
                .value_parameters
                .iter()
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
            .signature
            .value_parameters
            .iter()
            .map(|parameter| format!("{}: {}", parameter.name, state.type_name(parameter.ty)))
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
