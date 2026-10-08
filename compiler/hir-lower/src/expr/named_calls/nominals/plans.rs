use super::*;
use crate::constructor_resolution::ResolvedNominalConstructor;

impl Lowerer {
    pub(super) fn named_nominal_plans(
        &mut self,
        binding: &NamedCallBinding,
        call: &ast::CallExpr,
        expected: Option<TypeId>,
    ) -> Result<PreparedNominalPlans, ()> {
        let (nominal, expected, fixed_alias) = match binding.target {
            NamedCallTarget::Value(ValueTarget::Object(id)) => {
                let ty = self.object_types[self.objects[id].object_type].canonical_type;
                if !self.type_exposes_invoke(ty, false) {
                    let kind = match self.objects[id].kind {
                        hir::ObjectKind::Standalone => "object",
                        hir::ObjectKind::Companion(_) => "companion object",
                    };
                    self.error(
                        call.span,
                        format!("{kind} `{}` cannot be constructed", call.callee.text),
                    );
                    return Err(());
                }
                return Ok(PreparedNominalPlans::Nominal(Vec::new(), None));
            }
            NamedCallTarget::Type(TopLevelTypeTarget::Nominal(nominal)) => {
                (nominal, expected, false)
            }
            NamedCallTarget::Type(TopLevelTypeTarget::Alias(id)) => {
                let ty = self
                    .resolve_type_alias_id_reference(id, &call.callee, !call.type_args.is_empty())
                    .ok_or(())?;
                let Some(nominal) = self.nominal_target_for_type(ty) else {
                    if let Some(owner) = self.imported_nominal_owner(ty) {
                        return Ok(self.imported_nominal_plans(
                            owner,
                            call,
                            match owner {
                                hir::SourceNominalId::Concrete(_) => expected,
                                hir::SourceNominalId::GenericTemplate(_) => Some(ty),
                            },
                            true,
                        ));
                    }
                    self.error(
                        call.span,
                        format!(
                            "typealias `{}` does not name a constructible type",
                            call.callee.text
                        ),
                    );
                    return Err(());
                };
                (nominal, Some(ty), true)
            }
            NamedCallTarget::ImportedDependency(hir::ImportedTarget::GenericType(owner)) => {
                return Ok(self.imported_nominal_plans(
                    hir::SourceNominalId::GenericTemplate(owner.persistent()),
                    call,
                    expected,
                    false,
                ));
            }
            NamedCallTarget::ImportedDependency(
                hir::ImportedTarget::Type(_) | hir::ImportedTarget::TypeAlias(_),
            ) => {
                let NamedCallOrigin::Dependency(binding) = &binding.origin else {
                    unreachable!("a dependency type retains its name binding")
                };
                let ty = self
                    .resolve_imported_dependency_type_target(
                        binding,
                        &call.callee,
                        !call.type_args.is_empty(),
                    )
                    .ok_or(())?;
                let Some(owner) = self.imported_nominal_owner(ty) else {
                    self.error(
                        call.span,
                        format!(
                            "type `{}` does not name a constructible type",
                            call.callee.text
                        ),
                    );
                    return Err(());
                };
                return Ok(self.imported_nominal_plans(
                    owner,
                    call,
                    match owner {
                        hir::SourceNominalId::Concrete(_) => expected,
                        hir::SourceNominalId::GenericTemplate(_) => Some(ty),
                    },
                    true,
                ));
            }
            NamedCallTarget::Value(ValueTarget::Variant(target)) => {
                if self.resolved_variant_style(target) == VariantStyle::Unit {
                    self.error(call.span, format!("unit variant `{}` of `{}` does not take arguments; use `{}` without parentheses", call.callee.text, self.enums[target.enumeration()].name, call.callee.text));
                    return Err(());
                }
                return Ok(PreparedNominalPlans::Nominal(
                    vec![NominalPlan {
                        view: self.nominal_constructor_view(
                            NominalConstructorSource::Variant(target),
                            call.span,
                        ),
                        expected,
                        fixed_alias: false,
                    }],
                    None,
                ));
            }
            _ => return Ok(PreparedNominalPlans::Nominal(Vec::new(), None)),
        };
        let sources = match nominal {
            crate::NominalTarget::Struct(id) => {
                if Some(id) == self.ffi_foreign_callback {
                    self.error(
                        call.span,
                        "`ForeignCallback` values can only be produced by `foreignCallback`"
                            .to_string(),
                    );
                    return Err(());
                }
                if Some(id) == self.ffi_fun_ptr {
                    self.error(
                        call.span,
                        "intrinsic struct `FunPtr` has no source constructor".to_string(),
                    );
                    return Err(());
                }
                if Some(id) == self.ffi_ptr {
                    return Ok(PreparedNominalPlans::Nominal(
                        Vec::new(),
                        Some((id, expected, fixed_alias)),
                    ));
                }
                if matches!(
                    self.structs[id].representation,
                    hir::StructRepresentation::Intrinsic(_)
                ) {
                    self.error(
                        call.span,
                        format!(
                            "intrinsic struct `{}` has no source constructor",
                            self.structs[id].name
                        ),
                    );
                    return Err(());
                }
                self.structs[id]
                    .constructors
                    .iter()
                    .copied()
                    .map(NominalConstructorSource::Struct)
                    .collect::<Vec<_>>()
            }
            crate::NominalTarget::Class(id) => {
                if matches!(
                    self.classes[id].representation,
                    hir::ClassRepresentation::Intrinsic(hir::IntrinsicTypeKind::Atomic(_))
                ) {
                    return Ok(PreparedNominalPlans::Nominal(
                        vec![NominalPlan {
                            view: self.nominal_constructor_view(
                                NominalConstructorSource::Atomic(id),
                                call.span,
                            ),
                            expected,
                            fixed_alias,
                        }],
                        None,
                    ));
                }
                if self.array_class_kind(id).is_some() {
                    let view = self.nominal_constructor_view(
                        NominalConstructorSource::IntrinsicClass(id),
                        call.span,
                    );
                    return Ok(PreparedNominalPlans::Nominal(
                        vec![
                            NominalPlan {
                                view,
                                expected,
                                fixed_alias,
                            },
                            NominalPlan {
                                view: self.nominal_constructor_view(
                                    NominalConstructorSource::ArrayGenerate(id),
                                    call.span,
                                ),
                                expected,
                                fixed_alias,
                            },
                        ],
                        None,
                    ));
                }
                if self.classes[id].modifier == hir::ClassModifier::Abstract {
                    self.error(
                        call.span,
                        format!(
                            "abstract class `{}` cannot be instantiated",
                            self.classes[id].name
                        ),
                    );
                    return Err(());
                }
                if matches!(
                    self.classes[id].representation,
                    hir::ClassRepresentation::Intrinsic(_)
                ) {
                    self.error(
                        call.span,
                        format!(
                            "intrinsic class `{}` has no source constructor",
                            self.classes[id].name
                        ),
                    );
                    return Err(());
                }
                self.source_class_constructors(id)
                    .map(NominalConstructorSource::Class)
                    .collect::<Vec<_>>()
            }
            _ => {
                self.error(
                    call.span,
                    format!(
                        "type `{}` does not name a constructible type",
                        call.callee.text
                    ),
                );
                return Err(());
            }
        };
        let mut plans = Vec::new();
        for source in sources {
            if self.constructor_is_accessible(source) {
                plans.push(NominalPlan {
                    view: self.nominal_constructor_view(source, call.span),
                    expected,
                    fixed_alias,
                });
            }
        }
        if plans.is_empty() {
            self.error(
                call.span,
                format!(
                    "constructor of type `{}` is not accessible here",
                    call.callee.text
                ),
            );
            return Err(());
        }
        Ok(PreparedNominalPlans::Nominal(plans, None))
    }

    fn imported_nominal_plans(
        &mut self,
        owner: hir::SourceNominalId,
        call: &ast::CallExpr,
        expected: Option<TypeId>,
        fixed_alias: bool,
    ) -> PreparedNominalPlans {
        let declaration = self
            .dependencies
            .as_ref()
            .and_then(|dependencies| dependencies.nominal_declaration(owner))
            .expect("a resolved dependency type retains its declaration");
        if matches!(declaration.interface.source_shape(), hir::NominalSourceShapeV1::Intrinsic(representation)
            if matches!(representation.family(), hir::IntrinsicTypeKind::Atomic(_)))
        {
            return PreparedNominalPlans::Nominal(
                vec![NominalPlan {
                    view: self.nominal_constructor_view(
                        NominalConstructorSource::ImportedAtomic(owner),
                        call.span,
                    ),
                    expected,
                    fixed_alias: fixed_alias
                        && matches!(owner, hir::SourceNominalId::GenericTemplate(_)),
                }],
                None,
            );
        }
        if matches!(
            declaration.interface.source_shape(),
            hir::NominalSourceShapeV1::Intrinsic(representation)
                if matches!(representation.family(),
                    hir::IntrinsicTypeKind::Array | hir::IntrinsicTypeKind::MutableArray)
        ) {
            let view = self.nominal_constructor_view(
                NominalConstructorSource::ImportedArray(owner),
                call.span,
            );
            return PreparedNominalPlans::Nominal(
                vec![
                    NominalPlan {
                        view,
                        expected,
                        fixed_alias,
                    },
                    NominalPlan {
                        view: self.nominal_constructor_view(
                            NominalConstructorSource::ImportedArrayGenerate(owner),
                            call.span,
                        ),
                        expected,
                        fixed_alias,
                    },
                ],
                None,
            );
        }
        PreparedNominalPlans::Imported {
            owner,
            expected,
            fixed_alias,
        }
    }

    pub(super) fn named_nominal_expected_arguments(
        &self,
        definition: TypeId,
        expected: Option<TypeId>,
    ) -> Option<Vec<TypeId>> {
        match (&self.types[definition], &self.types[expected?]) {
            (Type::Class(a), Type::Class(b))
                if self.class_applications[*a].template == self.class_applications[*b].template =>
            {
                Some(self.class_applications[*b].arguments.clone())
            }
            (Type::Struct(a), Type::Struct(b))
                if self.struct_applications[*a].template
                    == self.struct_applications[*b].template =>
            {
                Some(self.struct_applications[*b].arguments.clone())
            }
            (Type::Enum(a), Type::Enum(b))
                if self.enum_applications[*a].template == self.enum_applications[*b].template =>
            {
                Some(self.enum_applications[*b].arguments.clone())
            }
            _ => None,
        }
    }

    fn array_generate_expression(&mut self, args: Vec<hir::Expr>) -> ExprKind {
        let [count, initializer]: [hir::Expr; 2] = args
            .try_into()
            .expect("the solved array generator has two materialized arguments");
        ExprKind::ArrayGenerate {
            count: Box::new(count),
            initializer: Box::new(initializer),
        }
    }

    pub(super) fn finish_named_nominal(
        &mut self,
        resolved: ResolvedNominalConstructor,
        span: Span,
    ) -> hir::Expr {
        let (kind, ty) = match resolved.source {
            NominalConstructorSource::Struct(source) => {
                let owner = self.struct_constructors[source].owner;
                let application = self.struct_application_id(owner, resolved.type_args);
                let constructor = self.struct_constructor_application(source, application);
                (
                    ExprKind::StructInit {
                        constructor,
                        args: resolved.args,
                    },
                    self.struct_applications[application].canonical_type,
                )
            }
            NominalConstructorSource::Class(source) => {
                let owner = self.class_constructors[source].owner;
                let application = self.class_application_id(owner, resolved.type_args);
                let constructor = self.class_constructor_application(source, application);
                (
                    ExprKind::ClassInit {
                        constructor,
                        args: resolved.args,
                    },
                    self.class_applications[application].canonical_type,
                )
            }
            NominalConstructorSource::Variant(target) => {
                let application =
                    self.enum_application_id(target.enumeration(), resolved.type_args);
                let variant = hir::AppliedEnumVariantRef::checked(
                    &self.enums,
                    &self.enum_applications,
                    self.nominal_identities
                        .as_ref()
                        .expect("nominal identities precede application references"),
                    application,
                    target,
                )
                .expect("the chosen nominal application owns its variant");
                (
                    ExprKind::VariantConstruct {
                        variant: self.enum_variant_reference(variant),
                        args: resolved.args,
                    },
                    self.enum_applications[application].canonical_type,
                )
            }
            NominalConstructorSource::ArrayGenerate(class) => {
                let ty = self.class_application(class, resolved.type_args);
                (self.array_generate_expression(resolved.args), ty)
            }
            NominalConstructorSource::ImportedArrayGenerate(owner) => {
                let ty = self
                    .imported_nominal_application(owner, resolved.type_args)
                    .expect("a solved array generator retains its complete application");
                (self.array_generate_expression(resolved.args), ty)
            }
            NominalConstructorSource::IntrinsicClass(class) => {
                let ty = self.class_application(class, resolved.type_args);
                let [argument]: [hir::Expr; 1] = resolved
                    .args
                    .try_into()
                    .expect("array conversion has one materialized argument");
                (ExprKind::ArrayClone(Box::new(argument)), ty)
            }
            NominalConstructorSource::Atomic(class) => {
                let ty = self.class_application(class, resolved.type_args);
                let [initial]: [hir::Expr; 1] = resolved
                    .args
                    .try_into()
                    .expect("an atomic constructor has one initial value");
                (ExprKind::AtomicNew(Box::new(initial)), ty)
            }
            NominalConstructorSource::ImportedAtomic(owner) => {
                let ty = self
                    .imported_nominal_application(owner, resolved.type_args)
                    .expect("a solved atomic constructor retains its complete application");
                let [initial]: [hir::Expr; 1] = resolved
                    .args
                    .try_into()
                    .expect("an atomic constructor has one initial value");
                (ExprKind::AtomicNew(Box::new(initial)), ty)
            }
            NominalConstructorSource::ImportedArray(owner) => {
                let ty = self
                    .imported_nominal_application(owner, resolved.type_args)
                    .expect("an applicable array conversion has a complete result type");
                let [argument]: [hir::Expr; 1] = resolved
                    .args
                    .try_into()
                    .expect("array conversion has one materialized argument");
                (ExprKind::ArrayClone(Box::new(argument)), ty)
            }
        };
        hir::Expr {
            kind,
            ty,
            span,
            origin: self.expression_origin(span),
        }
    }
}
