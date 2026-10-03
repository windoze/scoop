//! Source storage supplies one complete native-reference signature.

use super::*;
use crate::call_resolution::candidates::{CallableEffects, CallableView};
use crate::call_resolution::constraints::ConstraintFailure;
use crate::call_resolution::diagnostics::{
    callable_layer_name, callable_source_signature, render_callable_constraint_failure,
};
use crate::expr::named_calls::imported_dependency::{
    DependencySignature, ImportedCallableCandidate,
};

enum NativeReferenceSource {
    Local(Box<CallableView>),
    Dependency(
        Box<hir::ImportedDependencyCallableCandidate>,
        DependencySignature,
    ),
}

pub(super) struct NativeReferenceDeclaration {
    source: NativeReferenceSource,
    pub(super) effects: CallableEffects,
    pub(super) parameters: Vec<TypeId>,
    pub(super) return_type: TypeId,
    pub(super) display: String,
    pub(super) layer: &'static str,
}

pub(super) struct NativeReferenceFailure {
    pub(super) display: String,
    pub(super) layer: &'static str,
    pub(super) reason: String,
}

impl NativeReferenceDeclaration {
    pub(super) fn resolve(
        state: &mut Lowerer,
        binding: &NamedCallBinding,
        name: &str,
    ) -> Result<Self, NativeReferenceFailure> {
        match (&binding.target, &binding.origin) {
            (NamedCallTarget::Function(function), _) => {
                let extension = state.extension_receivers.contains_key(function);
                let view = state.callable_view(
                    &crate::CallableCandidate::function(*function, Vec::new()),
                    extension,
                );
                let display = callable_source_signature(state, name, &view);
                let layer = callable_layer_name(state, std::slice::from_ref(&view));
                let reason = eligibility(
                    state.functions[*function].method.is_some(),
                    extension,
                    !view.signature.callable_parameters.is_empty(),
                    view.effects,
                    matches!(state.functions[*function].kind, hir::FunctionKind::User(_)),
                );
                if let Some(reason) = reason {
                    return Err(NativeReferenceFailure {
                        display,
                        layer,
                        reason: reason.to_owned(),
                    });
                }
                Ok(Self {
                    parameters: view
                        .signature
                        .value_parameters
                        .iter()
                        .map(|parameter| parameter.ty)
                        .collect(),
                    return_type: view.signature.return_type,
                    effects: view.effects,
                    source: NativeReferenceSource::Local(Box::new(view)),
                    display,
                    layer,
                })
            }
            (NamedCallTarget::ImportedDependency(_), NamedCallOrigin::Dependency(binding)) => {
                let layer = "dependency top-level candidate";
                let fail = |reason: String| NativeReferenceFailure {
                    display: format!("dependency function `{name}`"),
                    layer,
                    reason,
                };
                let candidate = state
                    .dependencies
                    .as_ref()
                    .expect("dependency lookup retains its catalog")
                    .callable_candidate(binding)
                    .map_err(|error| fail(error.to_string()))?;
                let interface = candidate.interface();
                let effects = CallableEffects {
                    is_suspend: interface.effects().execution() == scoop_identity::Effect::Suspend,
                    attributes: interface.effects().function_attributes(),
                };
                let reason = eligibility(
                    matches!(interface.owner(), hir::PublicDeclarationOwnerV1::Nominal(_)),
                    interface.owner() == hir::PublicDeclarationOwnerV1::Extension,
                    !interface.type_parameters().is_empty(),
                    effects,
                    interface.effects().implementation() == hir::CallableImplementationV1::Scoop,
                );
                if let Some(reason) = reason {
                    return Err(fail(reason.to_owned()));
                }
                let signature = state
                    .imported_native_signature(&ImportedCallableCandidate::Binding(Box::new(
                        candidate.clone(),
                    )))
                    .map_err(|error| fail(error.diagnostic("native callback signature")))?;
                let parameters = signature
                    .value_parameters
                    .iter()
                    .map(|parameter| parameter.ty)
                    .collect();
                let arguments = signature
                    .value_parameters
                    .iter()
                    .map(|parameter| {
                        format!("{}: {}", parameter.name, state.type_name(parameter.ty))
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let display = format!(
                    "fun {name}({arguments}): {}",
                    state.type_name(signature.return_type)
                );
                Ok(Self {
                    parameters,
                    return_type: signature.return_type,
                    effects,
                    source: NativeReferenceSource::Dependency(Box::new(candidate), signature),
                    display,
                    layer,
                })
            }
            _ => unreachable!("native reference layers only contain callable declarations"),
        }
    }

    pub(super) fn failure(
        &self,
        state: &Lowerer,
        constraint: &ConstraintFailure,
    ) -> NativeReferenceFailure {
        let reason = match &self.source {
            NativeReferenceSource::Local(view) => {
                render_callable_constraint_failure(state, view, None, &[], constraint)
            }
            NativeReferenceSource::Dependency(_, signature) => state
                .render_imported_constraint_failure(
                    &signature.owner_parameters,
                    &signature.callable_parameters,
                    constraint,
                ),
        };
        NativeReferenceFailure {
            display: self.display.clone(),
            layer: self.layer,
            reason,
        }
    }

    pub(super) fn commit(self, state: &mut Lowerer) -> Result<hir::CallableTarget, String> {
        match self.source {
            NativeReferenceSource::Local(view) => {
                Ok(hir::Callable::Function(view.function()).into())
            }
            NativeReferenceSource::Dependency(candidate, _) => state
                .select_imported_dependency_callable_use(*candidate)
                .map(|(callee, _)| hir::CallableTarget::Dependency(callee))
                .map_err(|error| error.to_string()),
        }
    }
}

fn eligibility(
    member: bool,
    extension: bool,
    generic: bool,
    effects: CallableEffects,
    source_body: bool,
) -> Option<&'static str> {
    if member {
        Some("native addresses cannot target member functions")
    } else if extension {
        Some("native addresses cannot target extension functions")
    } else if generic {
        Some("native addresses cannot target generic functions")
    } else if effects.is_suspend {
        Some("native addresses cannot target suspend functions")
    } else if effects.attributes.gc_effect != hir::GcEffect::NoGc {
        Some("native callbacks must be declared `@NoGC`")
    } else if !source_body {
        Some("native addresses require a source function body")
    } else {
        None
    }
}
