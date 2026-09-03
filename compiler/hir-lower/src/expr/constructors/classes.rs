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
        let type_params = self.classes[class_id].type_params.clone();
        let explicit_type_args = self.resolve_call_type_args(type_arg_refs)?;
        if type_params.is_empty() && !explicit_type_args.is_empty() {
            self.error(span, format!("class `{name}` is not generic"));
            return None;
        }
        let mut bindings = vec![None; type_params.len()];
        if !self.bind_explicit_type_args(
            &mut bindings,
            0,
            &explicit_type_args,
            span,
            &format!("class `{name}`"),
        ) {
            return None;
        }
        if let Some(expected) = expected
            && let Type::Class(application) = self.types[expected]
            && self.class_applications[application].template == class_id
            && self.class_applications[application].arguments.len() == type_params.len()
        {
            let expected_args = self.class_applications[application].arguments.clone();
            for (binding, argument) in bindings.iter_mut().zip(expected_args) {
                if binding.is_none() {
                    *binding = Some(argument);
                }
            }
        }
        let property_types = props.iter().map(|(_, ty)| *ty).collect::<Vec<_>>();
        let inferred = self.lower_inference_args(args, &property_types, bindings, &type_params)?;
        let mut type_args = Vec::with_capacity(type_params.len());
        for (binding, parameter) in inferred.bindings.iter().copied().zip(&type_params) {
            let Some(argument) = binding else {
                self.error(
                    span,
                    format!(
                        "cannot infer type argument `{}` for class `{name}`",
                        parameter.name
                    ),
                );
                return None;
            };
            type_args.push(argument);
        }
        if !self.check_type_argument_kinds(
            &type_params,
            &type_args,
            span,
            &format!("class `{name}`"),
        ) {
            return None;
        }
        let lowered = inferred.finish(sink);
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
