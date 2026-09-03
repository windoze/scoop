use super::*;

mod arrays;
mod ffi;

impl Lowerer {
    /// A unit variant construction (`None`, `Color.Red`): the variant
    /// carries no fields, so the enum's type arguments (if any) must
    /// come from the expected-type hint — the M3 `None` inference
    /// rule, generalized.
    pub(super) fn lower_unit_variant(
        &mut self,
        name: &ast::Ident,
        enum_id: hir::EnumId,
        variant: u32,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let arity = self.enums[enum_id].type_params.len();
        let type_args = if arity == 0 {
            Vec::new()
        } else {
            let inferred = expected.and_then(|ty| match self.types[ty].clone() {
                Type::Enum(application) => {
                    let application = &self.enum_applications[application];
                    (application.template == enum_id && application.arguments.len() == arity)
                        .then(|| application.arguments.clone())
                }
                _ => None,
            });
            match inferred {
                Some(args) => args,
                None => {
                    self.error(
                        name.span,
                        format!("cannot infer the type of `{}`", name.text),
                    );
                    return None;
                }
            }
        };
        let application = self.enum_application_id(enum_id, type_args);
        let ty = self.enum_applications[application].canonical_type;
        Some(hir::Expr {
            kind: ExprKind::VariantConstruct {
                application,
                variant,
                args: Vec::new(),
            },
            ty,
            span: name.span,
        })
    }

    /// What a `Name` / `Name(...)` construction site resolves to. A
    /// dotted path `E.V` is always an enum variant; a bare name is a
    /// globally visible `Option` variant (`Some` / `None`), then a
    /// struct, then a class, then — for `Call` nodes only — a
    /// function.
    pub(super) fn classify_constructor(&mut self, name: &ast::Ident) -> Option<Constructor> {
        if let Some((enum_name, variant_name)) = name.text.split_once('.') {
            let Some(&enum_id) = self.enums_by_name.get(enum_name) else {
                self.error(name.span, format!("unknown enum `{enum_name}`"));
                return None;
            };
            let Some(variant) = self.find_variant(enum_id, variant_name) else {
                self.error(
                    name.span,
                    format!("enum `{enum_name}` has no variant `{variant_name}`"),
                );
                return None;
            };
            return Some(Constructor::Variant { enum_id, variant });
        }
        if let Some((enum_id, variant)) = self.option_variant(&name.text) {
            return Some(Constructor::Variant { enum_id, variant });
        }
        if let Some(&(struct_id, ty)) = self.structs_by_name.get(&name.text) {
            return Some(Constructor::Struct { struct_id, ty });
        }
        if let Some(&(class_id, _)) = self.classes_by_name.get(&name.text) {
            return Some(Constructor::Class { class_id });
        }
        Some(Constructor::Unmatched)
    }

    /// `Name(args...)` where `Name` is a class (M6): object
    /// construction with the class's own constructor properties
    /// (`hir::ExprKind::ClassInit`; base-class delegation is part of
    /// the generated constructor, mir-lower's job). Abstract classes
    /// cannot be instantiated. Argument count and types are checked
    /// against the constructor properties one by one (subtype
    /// adaptation included, mirroring struct construction).
    pub(super) fn lower_class_construct(
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

    /// Variant construction (`Some(x)`, `Shape.Circle(1)`,
    /// `E.WithDefault(1)` with a trailing default filled in). The
    /// variant behaves like a generic constructor function: type
    /// arguments are seeded from an expected `E<...>` hint and then
    /// inferred from the arguments (the same binding mechanism as
    /// generic calls), and each argument is checked against the
    /// instantiated field type.
    pub(super) fn lower_variant_construct(
        &mut self,
        enum_id: hir::EnumId,
        variant: u32,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let CallSite {
            type_args: type_arg_refs,
            args,
            span,
        } = call;
        let enum_name = self.enums[enum_id].name.clone();
        let type_params = self.enums[enum_id].type_params.clone();
        let variant_name = self.enums[enum_id].variants[variant as usize].name.clone();
        let fields: Vec<(String, TypeId)> = self.enums[enum_id].variants[variant as usize]
            .fields
            .iter()
            .map(|field| (field.name.clone(), field.ty))
            .collect();
        let total = fields.len();
        let supplied = args.len();
        if supplied > total {
            self.error(
                span,
                format!(
                    "variant `{variant_name}` of `{enum_name}` takes exactly {total} {}, but {supplied} were supplied",
                    if total == 1 { "argument" } else { "arguments" }
                ),
            );
            return None;
        }
        // Missing trailing fields must have constructor-style defaults.
        for index in supplied..total {
            if self.enums[enum_id].variants[variant as usize].defaults[index].is_none() {
                self.error(
                    span,
                    format!(
                        "variant `{variant_name}` of `{enum_name}` takes exactly {total} {}, but {supplied} were supplied",
                        if total == 1 { "argument" } else { "arguments" }
                    ),
                );
                return None;
            }
        }

        let explicit_type_args = self.resolve_call_type_args(type_arg_refs)?;
        let mut bindings = vec![None; type_params.len()];
        if !self.bind_explicit_type_args(
            &mut bindings,
            0,
            &explicit_type_args,
            span,
            &format!("enum `{enum_name}`"),
        ) {
            return None;
        }
        if let Some(expected) = expected {
            if let Type::Enum(application) = self.types[expected] {
                let application = self.enum_applications[application].clone();
                if application.template == enum_id
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
        let inferred =
            self.lower_inference_args(args, &field_tys[..supplied], bindings, &type_params)?;

        let mut type_args = Vec::with_capacity(inferred.bindings.len());
        for (binding, param) in inferred.bindings.iter().copied().zip(&type_params) {
            match binding {
                Some(ty) => type_args.push(ty),
                None => {
                    self.error(
                        span,
                        format!(
                            "cannot infer type argument `{}` for `{enum_name}.{variant_name}`",
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
            &format!("enum `{enum_name}`"),
        ) {
            return None;
        }
        let mut lowered = inferred.finish(sink);

        // Argument types must match the instantiated field types.
        for ((field_name, field_ty), arg) in fields.iter().zip(&lowered) {
            let expected = self.instantiate_ty(*field_ty, &type_args);
            if !self.types_equal(expected, arg.ty) {
                let expected_name = self.type_name(expected);
                let found = self.type_name(arg.ty);
                self.error(
                    arg.span,
                    format!(
                        "argument for field `{field_name}` of `{enum_name}.{variant_name}` must be of type {expected_name}, found {found}"
                    ),
                );
                return None;
            }
        }

        // Fill the trailing defaults (already lowered and type-checked
        // at the declaration site).
        for index in supplied..total {
            let default = self.enums[enum_id].variants[variant as usize].defaults[index]
                .as_ref()
                .expect("missing defaults were rejected above");
            lowered.push(clone_literal(default));
        }

        let application = self.enum_application_id(enum_id, type_args);
        let ty = self.enum_applications[application].canonical_type;
        Some(hir::Expr {
            kind: ExprKind::VariantConstruct {
                application,
                variant,
                args: lowered,
            },
            ty,
            span,
        })
    }

    /// Struct construction with positional arguments: argument count
    /// and types must match the declared fields one by one (the field
    /// type is the argument's expected-type hint).
    pub(super) fn lower_struct_init(
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
