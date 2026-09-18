use scoop_ast as ast;
use scoop_hir as hir;

use crate::{Lowerer, TypeId};

#[derive(Clone)]
pub(crate) struct ResolvedExtensionProperty {
    pub(crate) property: hir::PropertyId,
    pub(crate) receiver: hir::Expr,
    pub(crate) type_args: Vec<TypeId>,
    pub(crate) read: hir::Expr,
}

pub(crate) enum ExtensionPropertyResolution {
    NoCandidate,
    Failed,
    Resolved(Box<ResolvedExtensionProperty>),
}

pub(crate) enum ExtensionPropertyCandidateOutcome {
    NoCandidate,
    NoApplicable,
    Failed,
    Resolved(Box<ResolvedExtensionProperty>),
}

impl Lowerer {
    pub(crate) fn resolve_extension_property(
        &mut self,
        receiver: hir::Expr,
        name: &ast::Ident,
        sink: &mut Vec<hir::Statement>,
        require_read: bool,
    ) -> ExtensionPropertyResolution {
        let declared = self
            .top_level_namespaces
            .extension_property_layers(self.current_file, &name.text)
            .iter()
            .any(|properties| !properties.is_empty());
        let mut first_failure = None;
        for layer in self.named_extension_property_layers(&name.text) {
            let properties = layer
                .candidates
                .into_iter()
                .filter(|property| {
                    self.access_domain_allows(
                        &self.properties[*property].access.lookup.0,
                        Some(receiver.ty),
                    )
                })
                .collect::<Vec<_>>();
            if properties.is_empty() {
                continue;
            }
            let mut state = self.clone();
            let mut layer_sink = Vec::new();
            match state.resolve_extension_property_candidates_outcome(
                receiver.clone(),
                name,
                &properties,
                &mut layer_sink,
                require_read,
            ) {
                ExtensionPropertyCandidateOutcome::Resolved(resolved) => {
                    *self = state;
                    sink.extend(layer_sink);
                    return ExtensionPropertyResolution::Resolved(resolved);
                }
                ExtensionPropertyCandidateOutcome::NoApplicable => {
                    first_failure.get_or_insert(Box::new(state));
                }
                ExtensionPropertyCandidateOutcome::Failed => {
                    self.commit_layer_diagnostics(state);
                    return ExtensionPropertyResolution::Failed;
                }
                ExtensionPropertyCandidateOutcome::NoCandidate => {}
            }
        }
        if let Some(failure) = first_failure {
            self.commit_layer_diagnostics(*failure);
            ExtensionPropertyResolution::Failed
        } else if declared {
            self.error(
                name.span,
                format!("extension property `{}` is not accessible here", name.text),
            );
            ExtensionPropertyResolution::Failed
        } else {
            ExtensionPropertyResolution::NoCandidate
        }
    }

    pub(crate) fn resolve_extension_property_candidates_outcome(
        &mut self,
        receiver: hir::Expr,
        name: &ast::Ident,
        properties: &[hir::PropertyId],
        sink: &mut Vec<hir::Statement>,
        require_read: bool,
    ) -> ExtensionPropertyCandidateOutcome {
        if properties.is_empty() {
            return ExtensionPropertyCandidateOutcome::NoCandidate;
        }
        let getters = properties
            .iter()
            .map(|property| {
                match self.property_getters[self.properties[*property].capability.getter()]
                    .implementation
                {
                    hir::PropertyAccessorImplementation::Body(function) => function,
                    hir::PropertyAccessorImplementation::Storage
                    | hir::PropertyAccessorImplementation::Constant
                    | hir::PropertyAccessorImplementation::AbstractSlot(_) => {
                        unreachable!("extension properties have concrete getter bodies")
                    }
                }
            })
            .collect::<Vec<_>>();
        let no_type_args = [];
        let no_arguments = [];
        let resolved = match self.resolve_extension_overload_outcome(
            &name.text,
            &getters,
            receiver,
            crate::overload::OverloadCall {
                explicit_type_args: &no_type_args,
                arg_exprs: &no_arguments,
                span: name.span,
                expected_result: None,
                argument_protocol: crate::overload::CallArgumentProtocol::Ordinary,
            },
            sink,
        ) {
            crate::overload::OverloadResolutionOutcome::NoApplicable => {
                return ExtensionPropertyCandidateOutcome::NoApplicable;
            }
            crate::overload::OverloadResolutionOutcome::Blocked
            | crate::overload::OverloadResolutionOutcome::Failed => {
                return ExtensionPropertyCandidateOutcome::Failed;
            }
            crate::overload::OverloadResolutionOutcome::Resolved(resolved) => *resolved,
        };
        let function = resolved.function();
        let property = self.extension_property_by_getter[&function];
        let receiver = resolved
            .args
            .first()
            .cloned()
            .expect("an extension getter materializes its receiver argument");
        let callee = self.materialize_resolved_callee(&resolved);
        if require_read {
            self.check_call_effects(callee, name.span);
            let declaration = self.properties[property].clone();
            self.record_property_initialization_dependency(&declaration, name.span);
        }
        let read = hir::Expr {
            kind: hir::ExprKind::Call {
                callee,
                args: resolved.args,
            },
            ty: resolved.return_ty,
            span: name.span,
            origin: self.expression_origin(name.span),
        };
        ExtensionPropertyCandidateOutcome::Resolved(Box::new(ResolvedExtensionProperty {
            property,
            receiver,
            type_args: resolved.type_args,
            read,
        }))
    }

    pub(crate) fn lower_extension_property_write(
        &mut self,
        resolved: ResolvedExtensionProperty,
        value: hir::Expr,
        span: ast::Span,
    ) -> Option<hir::StatementKind> {
        let property = self.properties[resolved.property].clone();
        self.record_property_initialization_dependency(&property, span);
        let Some(setter) = property.capability.setter() else {
            self.error(
                span,
                format!("cannot assign to immutable property `{}`", property.name),
            );
            return None;
        };
        let setter = self.property_setters[setter].clone();
        if !self.access_domain_allows(&setter.access.lookup.0, Some(resolved.receiver.ty)) {
            self.error(
                span,
                format!("setter of property `{}` is not accessible", property.name),
            );
            return None;
        }
        let function = match setter.implementation {
            hir::PropertyAccessorImplementation::Body(function) => function,
            hir::PropertyAccessorImplementation::Storage
            | hir::PropertyAccessorImplementation::Constant
            | hir::PropertyAccessorImplementation::AbstractSlot(_) => {
                unreachable!("extension properties use concrete accessor functions")
            }
        };
        let candidate = crate::CallableCandidate::function(
            function,
            Vec::new(),
            self.function_lookup_witness(function),
        );
        let callee = self.materialize_candidate_callable(&candidate, &resolved.type_args);
        self.check_call_effects(callee, span);
        Some(hir::StatementKind::Expr(hir::Expr {
            kind: hir::ExprKind::Call {
                callee,
                args: vec![resolved.receiver, value],
            },
            ty: self.unit,
            span,
            origin: self.expression_origin(span),
        }))
    }
}
