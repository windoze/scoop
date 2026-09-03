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
        let props: Vec<(String, TypeId)> = self.classes[class_id]
            .semantic_constructor()
            .iter()
            .map(|field| (field.name.clone(), field.ty))
            .collect();
        if args.len() != props.len() {
            let expected = props.len();
            let supplied = args.len();
            let noun = if expected == 1 {
                "argument"
            } else {
                "arguments"
            };
            self.error(
                span,
                format!(
                    "class `{name}` takes exactly {expected} {noun}, but {supplied} were supplied"
                ),
            );
            return None;
        }
        let explicit_type_args = self.resolve_call_type_args(type_arg_refs)?;
        let type_param_count = self.classes[class_id].type_params.len();
        if type_param_count == 0 && !explicit_type_args.is_empty() {
            self.error(span, format!("class `{name}` is not generic"));
            return None;
        }
        let mut explicit_shape = vec![None; type_param_count];
        if !self.bind_explicit_type_args(
            &mut explicit_shape,
            0,
            &explicit_type_args,
            span,
            &format!("class `{name}`"),
        ) {
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
        let view = self.nominal_constructor_view(
            crate::call_resolution::candidates::NominalConstructorSource::Class(class_id),
        );
        let argument_map = crate::call_resolution::arguments::CandidateArgumentMap::positional(
            args.len(),
            crate::call_resolution::arguments::ReceiverInput::Absent,
        );
        let inferred = self.lower_nominal_arguments(
            NominalArgumentInput {
                view: &view,
                argument_map: &argument_map,
                expressions: args,
                explicit_type_args: &explicit_type_args,
                expected_type_args: expected_arguments.as_deref(),
                span,
            },
            sink,
        )?;
        let type_args = inferred.type_args;
        let lowered = inferred.args;
        let mut adapted = Vec::with_capacity(lowered.len());
        for ((prop_name, prop_ty), arg) in props.iter().zip(lowered) {
            let prop_ty = self.instantiate_ty(*prop_ty, &type_args);
            if !self.is_subtype(arg.ty, prop_ty) {
                let expected = self.type_name(prop_ty);
                let found = self.type_name(arg.ty);
                self.error(
                    arg.span,
                    format!(
                        "argument for field `{prop_name}` of `{name}` must be of type {expected}, found {found}"
                    ),
                );
                return None;
            }
            adapted.push(self.adapt_to(arg, prop_ty));
        }
        let application = self.class_application_id(class_id, type_args);
        let ty = self.class_applications[application].canonical_type;
        Some(hir::Expr {
            kind: ExprKind::ClassInit {
                application,
                args: adapted,
            },
            ty,
            span,
        })
    }
}
