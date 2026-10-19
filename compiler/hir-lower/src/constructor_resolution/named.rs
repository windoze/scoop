//! Nominal half of a mixed function-like candidate partition.

use super::*;

pub(crate) struct NamedNominalProbe {
    pub(super) alias: Option<crate::aliases::AliasTemplate>,
    pub(super) candidate: ApplicableConstructor,
    pub(super) comparison_owners: Vec<hir::TypeParamDecl>,
    pub(super) comparison_parameters: Vec<TypeId>,
}

impl NamedNominalProbe {
    pub(crate) fn requirement_effects(&self) -> hir::RequiredCallEffects {
        let state = &self.candidate.state;
        let (safety, gc_effect) = match self.candidate.source {
            NominalConstructorSource::Struct(id) => (
                state.struct_constructors[id].safety,
                state.struct_constructors[id].source_gc_effect(),
            ),
            NominalConstructorSource::Class(id) => {
                (state.class_constructors[id].safety, hir::GcEffect::Managed)
            }
            _ => (hir::Safety::Safe, hir::GcEffect::Managed),
        };
        hir::RequiredCallEffects {
            is_suspend: false,
            safety,
            gc_effect,
        }
    }

    pub(crate) fn forwarding(
        &self,
        state: &mut Lowerer,
    ) -> crate::call_resolution::specificity::OwnedDeclarationForwarding {
        use crate::call_resolution::specificity::DeclarationForwardingView;
        if let Some(alias) = &self.alias {
            let original = state.nominal_constructor_view(self.candidate.source, alias.name.span);
            let signature = state.alias_forwarding_signature(alias, &original.signature);
            let parameters = self
                .candidate
                .argument_map
                .forwarding_parameter_types(&signature.value_parameters);
            return DeclarationForwardingView::parameter_groups(
                &[],
                &signature.callable_parameters,
                &parameters,
            )
            .to_owned();
        }
        DeclarationForwardingView::nominal_parameters(
            &self.comparison_owners,
            &self.comparison_parameters,
        )
        .to_owned()
    }

    pub(crate) fn parameterized(&self) -> bool {
        self.alias
            .as_ref()
            .map_or(!self.comparison_owners.is_empty(), |alias| {
                alias.parameterized()
            })
    }
    pub(crate) fn defaults(&self) -> usize {
        self.candidate.argument_map.explicit_default_count()
    }
    pub(crate) fn vararg(&self) -> bool {
        self.candidate
            .view
            .signature
            .value_parameters
            .iter()
            .any(|parameter| parameter.is_vararg())
    }
    pub(crate) fn source_argument_numeric(
        &self,
        index: usize,
    ) -> Option<crate::call_resolution::specificity::NumericLiteralKind> {
        self.candidate
            .state
            .numeric_literal_kind(self.candidate.inferred.args[index].ty)
    }
    pub(crate) fn signature(&self, state: &Lowerer) -> String {
        nominal_source_signature(state, &self.candidate.view)
    }

    pub(crate) fn diagnostic_order(
        &self,
        state: &Lowerer,
    ) -> crate::call_resolution::named::DeclarationDiagnosticOrder {
        use crate::call_resolution::named::DeclarationDiagnosticOrder;
        let (file, span) = match self.candidate.view.target {
            NominalConstructorSource::Struct(constructor) => {
                let owner = state.struct_constructors[constructor].owner;
                (state.struct_files[&owner], state.structs[owner].span)
            }
            NominalConstructorSource::Class(constructor) => {
                let owner = state.class_constructors[constructor].owner;
                (state.class_files[&owner], state.classes[owner].span)
            }
            NominalConstructorSource::Atomic(class)
            | NominalConstructorSource::IntrinsicClass(class)
            | NominalConstructorSource::ArrayGenerate(class) => {
                (state.class_files[&class], state.classes[class].span)
            }
            NominalConstructorSource::ImportedAtomic(owner)
            | NominalConstructorSource::ImportedArray(owner)
            | NominalConstructorSource::ImportedArrayGenerate(owner) => {
                return DeclarationDiagnosticOrder::ImportedIntrinsic(owner);
            }
            NominalConstructorSource::Variant(variant) => {
                let owner = variant.enumeration();
                (state.enum_files[&owner], state.enums[owner].span)
            }
        };
        DeclarationDiagnosticOrder::Source(file, span.start, span.end)
    }

    pub(crate) fn fix_forwarding_parameters(&mut self, state: &mut Lowerer, arguments: &[TypeId]) {
        self.comparison_parameters = self
            .comparison_parameters
            .iter()
            .map(|&ty| state.instantiate_ty(ty, arguments))
            .collect();
        self.comparison_owners.clear();
    }
}

impl Lowerer {
    pub(crate) fn probe_named_nominal(
        &self,
        view: NominalConstructorView,
        call: NominalConstructorCall<'_>,
    ) -> Result<NamedNominalProbe, Box<Lowerer>> {
        let mut state = self.clone();
        let enclosing =
            match state.constructor_enclosing_instance(view.target, call.enclosing, call.span) {
                Ok(receiver) => receiver,
                Err(()) => return Err(Box::new(state)),
            };
        let Some(explicit) = state.constructor_type_arguments(
            enclosing.as_ref(),
            &view,
            call.explicit_type_args,
            call.span,
        ) else {
            return Err(Box::new(state));
        };
        if !explicit.is_empty() && explicit.len() != view.signature.owner_parameters.len() {
            state.diagnose_nominal_shape_failure(
                &view,
                call.span,
                format!(
                    "expects {} explicit type argument(s), but {} were supplied",
                    view.signature.owner_parameters.len(),
                    explicit.len()
                ),
            );
            return Err(Box::new(state));
        }
        let argument_map = match CandidateArgumentMap::source_nominal(&view, call.arguments) {
            Ok(map) => map,
            Err(reason) => {
                state.diagnose_nominal_shape_failure(&view, call.span, reason.describe());
                return Err(Box::new(state));
            }
        };
        let before = state.diagnostics.len();
        let inferred = state.lower_nominal_arguments(NominalArgumentInput {
            view: &view,
            argument_map: &argument_map,
            expressions: call.arguments,
            explicit_type_args: &explicit,
            expected_type_args: call.expected_type_args,
            span: call.span,
        });
        let Some(inferred) = inferred else {
            return Err(Box::new(state));
        };
        if state.diagnostics.len() != before {
            return Err(Box::new(state));
        }
        let mut parameter_types =
            argument_map.forwarding_parameter_types(&view.signature.value_parameters);
        let prefix = enclosing
            .as_ref()
            .filter(|receiver| matches!(state.types[receiver.ty], hir::Type::Class(_)))
            .map_or(
                state.local_constructor_parameter_count(view.target),
                |receiver| {
                    state
                        .nominal_application(receiver.ty)
                        .expect("an outer application")
                        .arguments
                        .len()
                },
            );
        let bindings = view
            .signature
            .owner_parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| {
                let ty = if index < prefix {
                    inferred.type_args[index]
                } else {
                    state.intern_type(hir::Type::Param(parameter.id))
                };
                (parameter.id, ty)
            })
            .collect::<Vec<_>>();
        for ty in &mut parameter_types {
            *ty = state.instantiate_method_ty(*ty, &bindings);
        }
        Ok(NamedNominalProbe {
            alias: None,
            comparison_owners: view.signature.owner_parameters[prefix..].to_vec(),
            comparison_parameters: parameter_types.clone(),
            candidate: ApplicableConstructor {
                enclosing,
                state: Box::new(state),
                source: view.target,
                view,
                argument_map,
                inferred,
                parameter_types,
            },
        })
    }

    pub(crate) fn commit_named_nominal(
        &mut self,
        probe: NamedNominalProbe,
        span: ast::Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<ResolvedNominalConstructor> {
        self.commit_nominal_candidate(probe.candidate, span, sink)
    }

    pub(super) fn commit_nominal_candidate(
        &mut self,
        winner: ApplicableConstructor,
        span: ast::Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<ResolvedNominalConstructor> {
        *self = *winner.state;
        self.check_constructor_call_safety(winner.source, span);
        let type_args = winner.inferred.type_args;
        let args = self.materialize_nominal_arguments(
            crate::argument_materialization::NominalArgumentMaterialization {
                receiver: winner.enclosing,
                view: &winner.view,
                argument_map: &winner.argument_map,
                type_args: &type_args,
                source_args: winner.inferred.args,
                argument_sinks: winner.inferred.argument_sinks,
                call_span: span,
            },
            sink,
        )?;
        Some(ResolvedNominalConstructor {
            source: winner.source,
            type_args,
            args,
        })
    }
}
