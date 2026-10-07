use super::*;

impl Lowerer {
    /// `Name(args...)` where `Name` is a class (M6): object
    /// construction against the complete typed primary/secondary candidate
    /// family. HIR fixes both the exact nominal application and source
    /// constructor; LocalConcrete later expands its checked initializer plan.
    /// Abstract classes cannot be instantiated.
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
        let constructors = self.source_class_constructors(class_id).collect::<Vec<_>>();
        if constructors.is_empty() {
            self.error(span, format!("class `{name}` has no source constructor"));
            return None;
        }
        let explicit_type_args = self.resolve_call_type_args(type_arg_refs)?;
        let type_param_count = self.classes[class_id].type_params.len();
        let expected_arguments = expected.and_then(|expected| {
            let Type::Class(application) = self.types[expected] else {
                return None;
            };
            let application = &self.class_applications[application];
            (application.template
                == self
                    .nominal_identity(crate::Owner::Class(class_id))
                    .declaration_id()
                && application.arguments.len() == type_param_count)
                .then(|| application.arguments.clone())
        });
        let candidates = constructors
            .iter()
            .copied()
            .map(crate::call_resolution::candidates::NominalConstructorSource::Class)
            .collect::<Vec<_>>();
        let resolved = self.resolve_nominal_constructor_overload(
            &name,
            &candidates,
            crate::constructor_resolution::NominalConstructorCall {
                explicit_type_args: &explicit_type_args,
                expected_type_args: expected_arguments.as_deref(),
                arguments: args,
                span,
            },
            sink,
        )?;
        let crate::call_resolution::candidates::NominalConstructorSource::Class(constructor) =
            resolved.source
        else {
            unreachable!("class construction has only class constructor candidates")
        };
        let type_args = resolved.type_args;
        let application = self.class_application_id(class_id, type_args);
        let constructor = self.class_constructor_application(constructor, application);
        let ty = self.class_applications[application].canonical_type;
        Some(hir::Expr {
            kind: ExprKind::ClassInit {
                constructor,
                args: resolved.args,
            },
            ty,
            span,
            origin: self.expression_origin(span),
        })
    }
}
