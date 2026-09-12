use super::*;

impl Lowerer {
    /// Resolve a class's constructor properties, base-class clause and
    /// interface list (pass 2).
    pub(crate) fn resolve_class(&mut self, id: ClassId, decl: &ast::ClassDecl) {
        self.type_params_in_scope = self.classes[id].type_params.clone();
        if matches!(
            self.classes[id].representation,
            hir::ClassRepresentation::Intrinsic(_)
        ) {
            for member in &decl.members {
                if !matches!(member, ast::ClassMember::Function(_)) {
                    self.error(
                        member.span(),
                        "an intrinsic class cannot declare stored properties, init blocks, or constructors"
                            .into(),
                    );
                }
            }
            let interfaces = self.resolve_supertype_interface_list(&decl.supertypes);
            self.classes[id].interfaces = interfaces;
            self.type_params_in_scope.clear();
            return;
        }
        let mut seen = std::collections::HashMap::new();
        let mut parameters = Vec::new();
        let mut fields = Vec::new();
        let mut field_names = std::collections::HashSet::new();
        let mut parameter_calling = Vec::new();
        for parameter in &decl.constructor {
            if let Some(previous) = seen.insert(parameter.name.text.clone(), parameter.property) {
                let message = if previous != ast::PrimaryParameterProperty::Plain
                    && parameter.property != ast::PrimaryParameterProperty::Plain
                {
                    format!(
                        "duplicate property `{}` in class `{}`",
                        parameter.name.text, decl.name.text
                    )
                } else {
                    format!(
                        "duplicate primary constructor parameter `{}` in class `{}`",
                        parameter.name.text, decl.name.text
                    )
                };
                self.error(parameter.name.span, message);
                continue;
            }
            let Some((ty, calling)) = self.resolve_parameter(&parameter.ty, &parameter.syntax)
            else {
                continue; // diagnostic already recorded
            };
            let lowered_parameter =
                self.constructor_parameter(parameter.name.text.clone(), ty, parameter.name.span);
            let parameter_id = lowered_parameter.id;
            parameters.push(lowered_parameter);
            if parameter.property != ast::PrimaryParameterProperty::Plain {
                field_names.insert(parameter.name.text.clone());
                let access = self.member_access(
                    parameter
                        .member_visibility
                        .unwrap_or(ast::VisibilitySyntax::Omitted),
                    parameter.name.span,
                    "primary-constructor property",
                    Owner::Class(id),
                    self.current_file,
                    if parameter.is_override {
                        crate::visibility::MemberSlotAccess::Override
                    } else {
                        crate::visibility::MemberSlotAccess::None
                    },
                );
                let field =
                    self.allocate_primary_class_property(id, parameter, ty, parameter_id, access);
                fields.push(field);
            }
            parameter_calling.push(calling);
        }
        for property in decl.members.iter().filter_map(|member| match member {
            ast::ClassMember::StoredProperty(property) => Some(property),
            _ => None,
        }) {
            if !field_names.insert(property.name.text.clone()) {
                self.error(
                    property.name.span,
                    format!(
                        "duplicate field `{}` in class `{}`",
                        property.name.text, decl.name.text
                    ),
                );
                continue;
            }
            let Some(ty) = self.resolve_type_ref(&property.ty) else {
                continue;
            };
            let slot_access = if property.is_override {
                crate::visibility::MemberSlotAccess::Override
            } else if property.modifier != ast::MethodModifier::Final {
                crate::visibility::MemberSlotAccess::Declared
            } else {
                crate::visibility::MemberSlotAccess::None
            };
            let access = self.member_access(
                property.visibility,
                property.name.span,
                "property",
                Owner::Class(id),
                self.current_file,
                slot_access,
            );
            if let Some(field) = self.allocate_class_property(id, property, ty, access) {
                fields.push(field);
            }
        }
        self.classes[id].representation = hir::ClassRepresentation::Declared;
        self.classes[id].fields = fields;
        let has_explicit_primary = !decl.constructor.is_omitted();
        let should_synthesize_primary =
            decl.constructor.is_omitted() && decl.secondary_constructors().next().is_none();
        if has_explicit_primary || should_synthesize_primary {
            let visibility = match &decl.constructor {
                ast::ClassConstructorDecl::Omitted => ast::VisibilitySyntax::Omitted,
                ast::ClassConstructorDecl::Declared(constructor) => constructor.visibility,
            };
            let access = self.member_access(
                visibility,
                decl.span,
                "constructor",
                Owner::Class(id),
                self.current_file,
                crate::visibility::MemberSlotAccess::None,
            );
            let constructor = self.class_constructors.alloc(hir::ClassConstructor {
                owner: id,
                identity_kind: hir::ClassConstructorIdentityKind::Source,
                access,
                parameters,
                kind: hir::ClassConstructorKind::Primary {
                    base: hir::BaseInitialization::Root,
                    primary_stores: Vec::new(),
                    common_initialization: Vec::new(),
                },
                span: decl.span,
                origin: self.definition_origin(decl.span),
            });
            self.classes[id].constructors.push(constructor);
            self.class_parameter_calling
                .insert(constructor, parameter_calling);
        }

        for source in decl.secondary_constructors() {
            let mut parameters = Vec::with_capacity(source.params.len());
            let mut callings = Vec::with_capacity(source.params.len());
            let mut names = std::collections::HashSet::new();
            for parameter in &source.params {
                if !names.insert(parameter.name.text.clone()) {
                    self.error(
                        parameter.name.span,
                        format!("duplicate constructor parameter `{}`", parameter.name.text),
                    );
                    continue;
                }
                let Some(resolved) = self.resolve_fn_param(parameter) else {
                    continue;
                };
                parameters.push(self.constructor_parameter(
                    resolved.name.text,
                    resolved.ty,
                    resolved.name.span,
                ));
                callings.push(resolved.calling);
            }
            let access = self.member_access(
                source.visibility,
                source.span,
                "constructor",
                Owner::Class(id),
                self.current_file,
                crate::visibility::MemberSlotAccess::None,
            );
            let constructor = self.class_constructors.alloc(hir::ClassConstructor {
                owner: id,
                identity_kind: hir::ClassConstructorIdentityKind::Source,
                access,
                parameters,
                kind: hir::ClassConstructorKind::Secondary {
                    delegation: hir::ClassSecondaryDelegation::Terminal {
                        base: hir::BaseInitialization::Root,
                        common_initialization: Vec::new(),
                    },
                    body: hir::Body {
                        locals: la_arena::Arena::new(),
                        statements: Vec::new(),
                    },
                },
                span: source.span,
                origin: self.definition_origin(source.span),
            });
            self.classes[id].constructors.push(constructor);
            self.class_parameter_calling.insert(constructor, callings);
        }

        self.resolve_class_supertypes(id, &decl.supertypes, "a class");
        self.type_params_in_scope.clear();
    }

    pub(crate) fn resolve_class_supertypes(
        &mut self,
        id: ClassId,
        specs: &[ast::SupertypeSpec],
        host: &str,
    ) {
        let mut interfaces = Vec::new();
        let mut base = None;
        for spec in specs {
            let Some(ty) = self.resolve_type_ref(&spec.ty) else {
                continue;
            };
            match self.types[ty] {
                Type::Class(application) => {
                    if base.is_some() {
                        self.error(
                            spec.span,
                            format!("{host} may have only one direct base class"),
                        );
                        continue;
                    }
                    let base_id = self.class_applications[application].template;
                    if self.classes[base_id].modifier == hir::ClassModifier::Final {
                        self.error(
                            spec.ty.span,
                            format!(
                                "class `{}` is final and cannot be inherited",
                                self.classes[base_id].name
                            ),
                        );
                        continue;
                    }
                    base = Some(ty);
                }
                Type::Interface(_) => {
                    if spec.constructor_arguments.is_some() {
                        self.error(
                            spec.span,
                            "interfaces cannot have constructor arguments".into(),
                        );
                    } else if !interfaces.iter().any(|&other| self.types_equal(other, ty)) {
                        interfaces.push(ty);
                    }
                }
                _ => self.error(
                    spec.ty.span,
                    format!("`{}` is not a class or interface", self.type_name(ty)),
                ),
            }
        }
        self.classes[id].base_class = base;
        self.classes[id].interfaces = interfaces;
    }

    /// Resolve an interface list (`: I1, I2`) on any declaration —
    /// classes, structs and enums share the rules (spec 9.1 / 4.4.3):
    /// every name must be an interface, duplicates are dropped.
    pub(crate) fn resolve_interface_list(&mut self, refs: &[ast::TypeRef]) -> Vec<TypeId> {
        let mut interfaces = Vec::new();
        for ty_ref in refs {
            let Some(ty) = self.resolve_type_ref(ty_ref) else {
                continue;
            };
            if !matches!(self.types[ty], Type::Interface(..)) {
                self.error(
                    ty_ref.span,
                    format!("`{}` is not an interface", self.type_name(ty)),
                );
                continue;
            }
            if !interfaces.iter().any(|&other| self.types_equal(other, ty)) {
                interfaces.push(ty);
            }
        }
        interfaces
    }

    pub(crate) fn resolve_supertype_interface_list(
        &mut self,
        specs: &[ast::SupertypeSpec],
    ) -> Vec<TypeId> {
        let mut interfaces = Vec::new();
        for spec in specs {
            if spec.constructor_arguments.is_some() {
                self.error(
                    spec.span,
                    "interfaces and value types cannot use supertype constructor arguments".into(),
                );
                continue;
            }
            let Some(ty) = self.resolve_type_ref(&spec.ty) else {
                continue;
            };
            if !matches!(self.types[ty], Type::Interface(..)) {
                self.error(
                    spec.ty.span,
                    format!("`{}` is not an interface", self.type_name(ty)),
                );
                continue;
            }
            if !interfaces.iter().any(|&other| self.types_equal(other, ty)) {
                interfaces.push(ty);
            }
        }
        interfaces
    }
}
