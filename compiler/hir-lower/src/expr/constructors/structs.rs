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
        let constructors = self.structs[struct_id].constructors.clone();
        let explicit_type_args = self.resolve_call_type_args(type_arg_refs)?;
        let type_param_count = self.structs[struct_id].type_params.len();
        let expected_arguments = expected.and_then(|expected| {
            let Type::Struct(application) = self.types[expected] else {
                return None;
            };
            let application = &self.struct_applications[application];
            (application.template
                == self
                    .nominal_identity(crate::Owner::Struct(struct_id))
                    .declaration_id()
                && application.arguments.len() == type_param_count)
                .then(|| application.arguments.clone())
        });
        let candidates = constructors
            .iter()
            .copied()
            .map(crate::call_resolution::candidates::NominalConstructorSource::Struct)
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
        let crate::call_resolution::candidates::NominalConstructorSource::Struct(
            source_constructor,
        ) = resolved.source
        else {
            unreachable!("struct construction has only struct constructor candidates")
        };
        let type_args = resolved.type_args;
        let application = self.struct_application_id(struct_id, type_args);
        let constructor = self.struct_constructor_application(source_constructor, application);
        let ty = self.struct_applications[application].canonical_type;
        debug_assert!(
            !self.structs[struct_id].type_params.is_empty() || self.types_equal(ty, definition_ty)
        );
        Some(hir::Expr {
            kind: ExprKind::StructInit {
                constructor,
                args: resolved.args,
            },
            ty,
            span,
            origin: self.expression_origin(span),
        })
    }
}
