use super::*;

impl Lowerer {
    /// `Name(args...)` where `Name` is a class (M6): object
    /// construction with the class's own constructor properties
    /// (`hir::ExprKind::ClassInit`; base-class delegation is part of
    /// the generated constructor, mir-lower's job). Abstract classes
    /// cannot be instantiated. Argument count and types are checked
    /// against the constructor properties one by one (subtype
    /// adaptation included, mirroring struct construction).
    pub(in crate::expr) fn lower_class_construct(
        &mut self,
        class_id: hir::ClassId,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let CallSite {
            type_args: type_arg_refs,
            args,
            span,
        } = call;
        let name = self.classes[class_id].name.clone();
        if matches!(
            self.classes[class_id].representation,
            hir::ClassRepresentation::Intrinsic(_)
        ) {
            self.error(
                span,
                format!("intrinsic class `{name}` has no source constructor"),
            );
            return None;
        }
        if self.classes[class_id].modifier == hir::ClassModifier::Abstract {
            self.error(
                span,
                format!("abstract class `{name}` cannot be instantiated"),
            );
            return None;
        }
        let view = self.nominal_constructor_view(
            crate::call_resolution::candidates::NominalConstructorSource::Class(class_id),
        );
        let argument_map =
            match crate::call_resolution::arguments::CandidateArgumentMap::source_nominal(
                &view, args,
            ) {
                Ok(argument_map) => argument_map,
                Err(failure) => {
                    self.diagnose_nominal_shape_failure(&view, span, failure.describe());
                    return None;
                }
            };
        let explicit_type_args = self.resolve_call_type_args(type_arg_refs)?;
        let type_param_count = view.owner_parameters.len();
        if !explicit_type_args.is_empty() && explicit_type_args.len() != type_param_count {
            self.diagnose_nominal_shape_failure(
                &view,
                span,
                format!(
                    "expects {type_param_count} explicit type argument(s), but {} were supplied",
                    explicit_type_args.len()
                ),
            );
            return None;
        }
        let expected_arguments = expected.and_then(|expected| {
            let Type::Class(application) = self.types[expected] else {
                return None;
            };
            let application = &self.class_applications[application];
            (application.template == class_id && application.arguments.len() == type_param_count)
                .then(|| application.arguments.clone())
        });
        let inferred = self.lower_nominal_arguments(NominalArgumentInput {
            view: &view,
            argument_map: &argument_map,
            expressions: args,
            explicit_type_args: &explicit_type_args,
            expected_type_args: expected_arguments.as_deref(),
            span,
        })?;
        let type_args = inferred.type_args;
        let adapted = self.materialize_nominal_arguments(
            crate::argument_materialization::NominalArgumentMaterialization {
                view: &view,
                argument_map: &argument_map,
                type_args: &type_args,
                source_args: inferred.args,
                argument_sinks: inferred.argument_sinks,
                call_span: span,
            },
            sink,
        );
        let application = self.class_application_id(class_id, type_args);
        let ty = self.class_applications[application].canonical_type;
        Some(hir::Expr {
            kind: ExprKind::ClassInit {
                application,
                args: adapted,
            },
            ty,
            span,
            origin: self.expression_origin(span),
        })
    }
}
