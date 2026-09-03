use super::*;

impl Lowerer {
    /// Struct construction with positional arguments: argument count
    /// and types must match the declared fields one by one (the field
    /// type is the argument's expected-type hint).
    pub(in crate::expr) fn lower_struct_init(
        &mut self,
        struct_id: hir::StructId,
        definition_ty: TypeId,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let CallSite {
            type_args: type_arg_refs,
            args,
            span,
        } = call;
        let name = self.structs[struct_id].name.clone();
        if matches!(
            self.structs[struct_id].representation,
            hir::StructRepresentation::Intrinsic(_)
        ) {
            self.error(
                span,
                format!("intrinsic struct `{name}` has no source constructor"),
            );
            return None;
        }
        let type_params = self.structs[struct_id].type_params.clone();
        let fields: Vec<(String, TypeId)> = self.structs[struct_id]
            .semantic_fields()
            .iter()
            .map(|field| (field.name.clone(), field.ty))
            .collect();
        if args.len() != fields.len() {
            let expected = fields.len();
            let supplied = args.len();
            let noun = if expected == 1 {
                "argument"
            } else {
                "arguments"
            };
            self.error(
                span,
                format!(
                    "struct `{name}` takes exactly {expected} {noun}, but {supplied} were supplied"
                ),
            );
            return None;
        }
        let explicit_type_args = self.resolve_call_type_args(type_arg_refs)?;
        let mut bindings = vec![None; type_params.len()];
        if !self.bind_explicit_type_args(
            &mut bindings,
            0,
            &explicit_type_args,
            span,
            &format!("struct `{name}`"),
        ) {
            return None;
        }
        if let Some(expected) = expected {
            if let Type::Struct(application) = self.types[expected] {
                let application = self.struct_applications[application].clone();
                if application.template == struct_id
                    && application.arguments.len() == type_params.len()
                {
                    for (binding, arg) in bindings.iter_mut().zip(application.arguments) {
                        if binding.is_none() {
                            *binding = Some(arg);
                        }
                    }
                }
            }
        }

        let field_tys: Vec<TypeId> = fields.iter().map(|(_, ty)| *ty).collect();
        let inferred = self.lower_inference_args(args, &field_tys, bindings, &type_params)?;

        let mut type_args = Vec::with_capacity(inferred.bindings.len());
        for (binding, param) in inferred.bindings.iter().copied().zip(&type_params) {
            match binding {
                Some(ty) => type_args.push(ty),
                None => {
                    self.error(
                        span,
                        format!(
                            "cannot infer type argument `{}` for struct `{name}`",
                            param.name
                        ),
                    );
                    return None;
                }
            }
        }
        if !self.check_type_argument_kinds(
            &type_params,
            &type_args,
            span,
            &format!("struct `{name}`"),
        ) {
            return None;
        }
        let lowered = inferred.finish(sink);

        let mut adapted = Vec::with_capacity(lowered.len());
        for ((field_name, field_ty), arg) in fields.iter().zip(lowered) {
            let field_ty = self.instantiate_ty(*field_ty, &type_args);
            if !self.is_subtype(arg.ty, field_ty) {
                let expected = self.type_name(field_ty);
                let found = self.type_name(arg.ty);
                self.error(
                    arg.span,
                    format!(
                        "argument for field `{field_name}` of `{name}` must be of type {expected}, found {found}"
                    ),
                );
                return None;
            }
            adapted.push(self.adapt_to(arg, field_ty));
        }
        let application = self.struct_application_id(struct_id, type_args);
        let ty = self.struct_applications[application].canonical_type;
        debug_assert!(
            !self.structs[struct_id].type_params.is_empty() || self.types_equal(ty, definition_ty)
        );
        Some(hir::Expr {
            kind: ExprKind::StructInit {
                application,
                args: adapted,
            },
            ty,
            span,
        })
    }
}
