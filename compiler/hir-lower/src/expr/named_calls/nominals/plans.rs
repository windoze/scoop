mod aliases;

use super::*;

impl Lowerer {
    pub(in crate::expr) fn named_nominal_plans(
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
                let alias = self.source_alias_template(id, &call.callee).ok_or(())?;
                return self.named_alias_plans(alias, call, expected);
            }
            NamedCallTarget::ImportedDependency(hir::ImportedTarget::TypeAlias(_)) => {
                let NamedCallOrigin::Dependency(binding) = &binding.origin else {
                    unreachable!("a dependency alias has its name binding");
                };
                let alias = self
                    .imported_alias_template(binding, &call.callee)
                    .ok_or(())?;
                return self.named_alias_plans(alias, call, expected);
            }
            NamedCallTarget::ImportedDependency(hir::ImportedTarget::GenericType(owner)) => {
                return Ok(self.imported_nominal_plans(
                    hir::SourceNominalId::GenericTemplate(owner.persistent()),
                    call,
                    expected,
                    false,
                ));
            }
            NamedCallTarget::ImportedDependency(hir::ImportedTarget::Type(_)) => {
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
                        alias: None,
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
        self.nominal_constructor_plans(nominal, call, expected, fixed_alias)
    }

    pub(in crate::expr) fn nominal_constructor_plans(
        &mut self,
        nominal: crate::NominalTarget,
        call: &ast::CallExpr,
        expected: Option<TypeId>,
        fixed_alias: bool,
    ) -> Result<PreparedNominalPlans, ()> {
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
                        Some((id, expected, fixed_alias, None)),
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
                            alias: None,
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
                                alias: None,
                                view,
                                expected,
                                fixed_alias,
                            },
                            NominalPlan {
                                alias: None,
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
                    alias: None,
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
                    alias: None,
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
                        alias: None,
                        view,
                        expected,
                        fixed_alias,
                    },
                    NominalPlan {
                        alias: None,
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
            alias: None,
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
}
