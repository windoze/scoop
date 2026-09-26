use super::*;
use crate::call_resolution::candidates::ValueParameter;
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
                return Ok(PreparedNominalPlans::Local(Vec::new(), None));
            }
            NamedCallTarget::Type(TopLevelTypeTarget::Nominal(nominal)) => {
                (nominal, expected, false)
            }
            NamedCallTarget::Type(TopLevelTypeTarget::Alias(id)) => {
                let ty = self
                    .resolve_type_alias_id_reference(id, &call.callee, !call.type_args.is_empty())
                    .ok_or(())?;
                let Some(nominal) = self.nominal_target_for_type(ty) else {
                    if let Some(owner) = self.imported_nominal_declaration(ty) {
                        return Ok(PreparedNominalPlans::Imported(owner));
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
                let Some(owner) = self.imported_nominal_declaration(ty) else {
                    self.error(
                        call.span,
                        format!(
                            "type `{}` does not name a constructible type",
                            call.callee.text
                        ),
                    );
                    return Err(());
                };
                return Ok(PreparedNominalPlans::Imported(owner));
            }
            NamedCallTarget::Value(ValueTarget::Variant(target)) => {
                if self.resolved_variant_style(target) == VariantStyle::Unit {
                    self.error(call.span, format!("unit variant `{}` of `{}` does not take arguments; use `{}` without parentheses", call.callee.text, self.enums[target.enumeration()].name, call.callee.text));
                    return Err(());
                }
                return Ok(PreparedNominalPlans::Local(
                    vec![NominalPlan {
                        view: self
                            .nominal_constructor_view(NominalConstructorSource::Variant(target)),
                        expected,
                        fixed_alias: false,
                    }],
                    None,
                ));
            }
            _ => return Ok(PreparedNominalPlans::Local(Vec::new(), None)),
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
                if Some(id) == self.ffi_ptr || Some(id) == self.ffi_fun_ptr {
                    return Ok(PreparedNominalPlans::Local(
                        Vec::new(),
                        Some((id, expected)),
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
                if let Some(kind) = self.array_class_kind(id) {
                    let mut view =
                        self.nominal_constructor_view(NominalConstructorSource::IntrinsicClass(id));
                    let element = self.intern_type(Type::Param(view.owner_parameters[0].id));
                    let opposite = match kind {
                        ArrayKind::Immutable => ArrayKind::Mutable,
                        ArrayKind::Mutable => ArrayKind::Immutable,
                    };
                    let ty = self.array_type(opposite, element);
                    view.value_parameters = vec![ValueParameter {
                        name: "source".into(),
                        calling: crate::defaults::SourceParameterCalling::Required,
                        ty,
                    }];
                    return Ok(PreparedNominalPlans::Local(
                        vec![NominalPlan {
                            view,
                            expected,
                            fixed_alias,
                        }],
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
                self.classes[id]
                    .constructors
                    .iter()
                    .copied()
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
                    view: self.nominal_constructor_view(source),
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
        Ok(PreparedNominalPlans::Local(plans, None))
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
                    application,
                    target,
                )
                .expect("the chosen nominal application owns its variant");
                (
                    ExprKind::VariantConstruct {
                        variant,
                        args: resolved.args,
                    },
                    self.enum_applications[application].canonical_type,
                )
            }
            NominalConstructorSource::IntrinsicClass(class) => {
                let ty = self.class_application(class, resolved.type_args);
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
