//! Nominal half of a mixed function-like candidate partition.

use super::*;

pub(crate) struct NamedNominalProbe {
    pub(super) candidate: ApplicableConstructor,
    comparison_owners: Vec<hir::TypeParamDecl>,
    comparison_parameters: Vec<TypeId>,
}

impl NamedNominalProbe {
    pub(crate) fn forwarding(
        &self,
    ) -> crate::call_resolution::specificity::DeclarationForwardingView<'_> {
        crate::call_resolution::specificity::DeclarationForwardingView::nominal_parameters(
            &self.comparison_owners,
            &self.comparison_parameters,
        )
    }

    pub(crate) fn parameterized(&self) -> bool {
        !self.comparison_owners.is_empty()
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
    pub(crate) fn source_argument_integer(&self, index: usize) -> Option<hir::IntegerKind> {
        match self.candidate.state.types[self.candidate.inferred.args[index].ty] {
            hir::Type::Integer(kind) => Some(kind),
            _ => None,
        }
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
            NominalConstructorSource::IntrinsicClass(class) => {
                (state.class_files[&class], state.classes[class].span)
            }
            NominalConstructorSource::ImportedArray(owner) => {
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
        if !call.explicit_type_args.is_empty()
            && call.explicit_type_args.len() != view.signature.owner_parameters.len()
        {
            state.diagnose_nominal_shape_failure(
                &view,
                call.span,
                format!(
                    "expects {} explicit type argument(s), but {} were supplied",
                    view.signature.owner_parameters.len(),
                    call.explicit_type_args.len()
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
            explicit_type_args: call.explicit_type_args,
            expected_type_args: call.expected_type_args,
            span: call.span,
        });
        let Some(inferred) = inferred else {
            return Err(Box::new(state));
        };
        if state.diagnostics.len() != before {
            return Err(Box::new(state));
        }
        let parameter_types =
            argument_map.forwarding_parameter_types(&view.signature.value_parameters);
        Ok(NamedNominalProbe {
            comparison_owners: view.signature.owner_parameters.clone(),
            comparison_parameters: parameter_types.clone(),
            candidate: ApplicableConstructor {
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
