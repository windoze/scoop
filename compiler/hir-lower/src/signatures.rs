use super::*;

mod functions;
mod parameters;
mod properties;

impl Lowerer {
    /// Resolve the field types of a struct declaration. Fields with
    /// duplicate names or unresolvable types are diagnosed and dropped;
    /// the module is rejected anyway once any diagnostic is recorded.
    pub(super) fn resolve_fields(&mut self, id: StructId, decl: ast::StructSource<'_>) {
        self.type_params_in_scope = self.structs[id].type_params.clone();
        if matches!(
            self.structs[id].representation,
            hir::StructRepresentation::Intrinsic(_)
        ) {
            for constructor in decl.secondary_constructors() {
                self.error(
                    constructor.span,
                    "an intrinsic struct cannot declare constructors".into(),
                );
            }
            self.resolve_struct_properties(id, decl, &mut HashSet::new());
            self.type_params_in_scope.clear();
            return;
        }
        let mut seen = HashSet::new();
        let mut fields = Vec::new();
        let mut field_spans = Vec::new();
        let mut field_name_spans = Vec::new();
        let mut parameter_calling = Vec::new();
        if decl.fields.is_omitted() {
            self.error(
                decl.name.span,
                "an omitted struct representation requires a validated intrinsic type declaration"
                    .to_string(),
            );
        }
        for field in &decl.fields {
            if !seen.insert(field.name.text.clone()) {
                self.error(
                    field.name.span,
                    format!(
                        "duplicate field `{}` in struct `{}`",
                        field.name.text, decl.name.text
                    ),
                );
                continue;
            }
            let Some((ty, calling)) = self.resolve_parameter(&field.ty, &field.syntax) else {
                continue; // diagnostic already recorded
            };
            fields.push(hir::Field {
                name: field.name.text.clone(),
                ty,
            });
            field_spans.push(field.span);
            field_name_spans.push(field.name.span);
            parameter_calling.push(calling);
        }
        self.structs[id].representation = hir::StructRepresentation::Declared(fields);
        for (index, span) in field_spans.into_iter().enumerate() {
            let field = hir::StructFieldRef::checked(&self.structs, id, index as u32)
                .expect("a resolved struct field index belongs to its declaration");
            assert!(self.struct_field_spans.insert(field, span).is_none());
        }
        for (index, field) in self.structs[id]
            .semantic_fields()
            .iter()
            .cloned()
            .enumerate()
            .collect::<Vec<_>>()
        {
            let field_ref = hir::StructFieldRef::checked(&self.structs, id, index as u32)
                .expect("a resolved struct field index belongs to its declaration");
            let field_span = self.struct_field_spans[&field_ref];
            let access = self.fixed_representation_access(Owner::Struct(id));
            let getter = self.property_getters.alloc(hir::PropertyGetter {
                access: access.clone(),
                implementation: hir::PropertyAccessorImplementation::Storage,
                attributes: hir::FunctionAttributes::default(),
                span: field_span,
            });
            let property = self.properties.alloc(hir::Property {
                owner: hir::PropertyOwner::Struct(id),
                name: field.name,
                access,
                modifier: hir::MethodModifier::Final,
                is_override: false,
                overrides: Vec::new(),
                ty: field.ty,
                capability: hir::PropertyCapability::ReadOnly { getter },
                representation: hir::PropertyRepresentation::Stored(hir::StoredProperty {
                    backing: hir::PropertyBacking::StructField {
                        owner: id,
                        index: index as u32,
                    },
                }),
                span: field_span,
            });
            self.structs[id].properties.push(property);
            self.property_name_spans
                .insert(property, field_name_spans[index]);
        }
        self.resolve_struct_properties(id, decl, &mut seen);
        let fields = self.structs[id]
            .semantic_fields()
            .iter()
            .enumerate()
            .map(|(index, field)| {
                let field_ref = hir::StructFieldRef::checked(&self.structs, id, index as u32)
                    .expect("a primary-constructor parameter belongs to its source field");
                (
                    field.name.clone(),
                    field.ty,
                    self.struct_field_spans[&field_ref],
                )
            })
            .collect::<Vec<_>>();
        let parameters = fields
            .into_iter()
            .map(|(name, ty, span)| self.constructor_parameter(name, ty, span))
            .collect();
        let access = self.fixed_representation_access(Owner::Struct(id));
        let constructor = self.struct_constructors.alloc(hir::StructConstructor {
            release_callability: Default::default(),
            owner: id,
            access,
            safety: hir::Safety::Safe,
            no_gc_type_params: Vec::new(),
            parameters,
            kind: hir::StructConstructorKind::Primary,
            span: decl.span,
            origin: self.definition_origin(decl.span),
        });
        self.structs[id].constructors.push(constructor);
        self.struct_parameter_calling
            .insert(constructor, parameter_calling);
        let primary_application =
            self.struct_constructor_application(constructor, self.structs[id].self_application);
        for source in decl.secondary_constructors() {
            let mut parameters = Vec::with_capacity(source.params.len());
            let mut callings = Vec::with_capacity(source.params.len());
            let mut names = HashSet::new();
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
                Owner::Struct(id),
                self.current_file,
                crate::visibility::MemberSlotAccess::None,
            );
            let safety = self.constructor_safety(&source.annotations, source.span);
            let gc_effect = self.constructor_gc_effect(&source.annotations, true);
            let constructor = self.struct_constructors.alloc(hir::StructConstructor {
                release_callability: Default::default(),
                owner: id,
                access,
                safety,
                no_gc_type_params: Vec::new(),
                parameters,
                kind: hir::StructConstructorKind::Secondary {
                    gc_effect,
                    delegation: hir::StructConstructorDelegation {
                        target: primary_application,
                        arguments: hir::ConstructorArguments {
                            locals: la_arena::Arena::new(),
                            statements: Vec::new(),
                            args: Vec::new(),
                        },
                    },
                    body: hir::Body {
                        locals: la_arena::Arena::new(),
                        statements: Vec::new(),
                    },
                },
                span: source.span,
                origin: self.definition_origin(source.span),
            });
            self.structs[id].constructors.push(constructor);
            self.struct_parameter_calling.insert(constructor, callings);
        }
        self.type_params_in_scope.clear();
    }

    /// Resolve the variants of an enum declaration (pass 2): duplicate
    /// variant and field names are diagnosed, field types resolve in
    /// the enum's type-parameter scope. Constructor-style defaults are
    /// type-checked into hygienic templates by the source-interface pass.
    pub(super) fn resolve_variants(&mut self, id: EnumId, decl: ast::EnumSource<'_>) {
        self.type_params_in_scope = self.enums[id].type_params.clone();
        let mut seen = HashSet::new();
        let mut variants = Vec::new();
        let mut resolved_source_indices = Vec::new();
        let mut member_spans = Vec::new();
        for (index, variant) in decl.variants.iter().enumerate() {
            if !seen.insert(variant.name.text.clone()) {
                self.error(
                    variant.name.span,
                    format!(
                        "duplicate variant `{}` in enum `{}`",
                        variant.name.text, decl.name.text
                    ),
                );
                continue;
            }
            let integral = self.enums[id].representation.integer_kind().is_some();
            let constant = matches!(variant.kind, ast::VariantDeclKind::Constant(_));
            if integral != constant {
                self.error(variant.span, if integral {
                    "integral enum variants require an explicit integer constant and cannot have payload"
                } else {
                    "explicit enum values require a primitive integer parent"
                }.into());
                continue;
            }
            let style = match &variant.kind {
                ast::VariantDeclKind::Unit | ast::VariantDeclKind::Constant(_) => {
                    hir::VariantStyle::Unit
                }
                ast::VariantDeclKind::Positional(_) => hir::VariantStyle::Positional,
                ast::VariantDeclKind::Named(_) => hir::VariantStyle::Named,
                ast::VariantDeclKind::Constructor(_) => hir::VariantStyle::Constructor,
            };
            let Some(resolved) = self.resolve_variant_fields(variant) else {
                continue; // diagnostic already recorded
            };
            let source_index = u32::try_from(index).expect("source variant index exceeds u32");
            let resolved_index =
                u32::try_from(variants.len()).expect("resolved variant index exceeds u32");
            self.variant_parameter_calling
                .insert((id, resolved_index), resolved.calling);
            resolved_source_indices.push((source_index, resolved_index));
            member_spans.push((resolved_index, variant.span, resolved.spans));
            variants.push(hir::Variant {
                name: variant.name.text.clone(),
                style,
                fields: resolved.fields,
            });
        }
        self.enums[id].variants = variants;
        for (variant_index, variant_span, field_spans) in member_spans {
            let variant = hir::EnumVariantRef::checked(&self.enums, id, variant_index)
                .expect("a resolved enum variant index belongs to its declaration");
            assert!(
                self.enum_variant_spans
                    .insert(variant, variant_span)
                    .is_none()
            );
            for (field_index, field_span) in field_spans.into_iter().enumerate() {
                let field =
                    hir::EnumVariantFieldRef::checked(&self.enums, variant, field_index as u32)
                        .expect("a resolved enum field index belongs to its variant");
                assert!(
                    self.enum_variant_field_spans
                        .insert(field, field_span)
                        .is_none()
                );
            }
        }
        for (source_index, resolved_index) in resolved_source_indices {
            let target = hir::EnumVariantRef::checked(&self.enums, id, resolved_index)
                .expect("a resolved variant index belongs to its enum");
            self.imports.bind_variant(id, source_index, target);
        }
        let mut properties = HashSet::new();
        for property in decl.properties {
            if !properties.insert(property.name.text.clone()) {
                self.error(
                    property.name.span,
                    format!(
                        "duplicate property `{}` in enum `{}`",
                        property.name.text, decl.name.text
                    ),
                );
                continue;
            }
            let Some(ty) = self.resolve_property_annotation(property) else {
                continue;
            };
            let access = self.member_access(
                property.visibility,
                property.name.span,
                "property",
                Owner::Enum(id),
                self.current_file,
                if property.is_override {
                    crate::visibility::MemberSlotAccess::Override
                } else {
                    crate::visibility::MemberSlotAccess::None
                },
            );
            self.allocate_value_property(Owner::Enum(id), property, ty, access);
        }
        self.type_params_in_scope.clear();
    }

    /// The fields of one variant.
    /// Returns `None` after recording a diagnostic.
    fn resolve_variant_fields(&mut self, variant: &ast::VariantDecl) -> Option<ResolvedFields> {
        match &variant.kind {
            ast::VariantDeclKind::Unit | ast::VariantDeclKind::Constant(_) => {
                Some(ResolvedFields::default())
            }
            // Positional fields get `_1`-style names (hir docs).
            ast::VariantDeclKind::Positional(types) => {
                let mut resolved = ResolvedFields::default();
                for (index, ty_ref) in types.iter().enumerate() {
                    let ty = self.resolve_type_ref(&ty_ref.ty)?;
                    resolved.fields.push(hir::Field {
                        name: format!("_{}", index + 1),
                        ty,
                    });
                    resolved.spans.push(ty_ref.span);
                    resolved.calling.push(FnParamCalling::Required);
                }
                Some(resolved)
            }
            ast::VariantDeclKind::Named(fields) | ast::VariantDeclKind::Constructor(fields) => {
                let constructor = matches!(variant.kind, ast::VariantDeclKind::Constructor(_));
                let mut seen = HashSet::new();
                let mut resolved = ResolvedFields::default();
                for field in fields {
                    if !seen.insert(field.name.text.clone()) {
                        self.error(
                            field.name.span,
                            format!(
                                "duplicate field `{}` in variant `{}`",
                                field.name.text, variant.name.text
                            ),
                        );
                        return None;
                    }
                    let (ty, calling) = self.resolve_parameter(&field.ty, &field.syntax)?;
                    if !constructor
                        && matches!(
                            field.syntax,
                            ast::ParameterSyntax::Default { .. }
                                | ast::ParameterSyntax::Vararg { .. }
                        )
                    {
                        self.error(
                            field.span,
                            format!(
                                "default value of field `{}` in variant `{}` is only allowed on constructor-style variants",
                                field.name.text, variant.name.text
                            ),
                        );
                        return None;
                    }
                    resolved.fields.push(hir::Field {
                        name: field.name.text.clone(),
                        ty,
                    });
                    resolved.spans.push(field.span);
                    resolved.calling.push(calling);
                }
                Some(resolved)
            }
        }
    }
}

/// Intermediate result of variant field resolution.
#[derive(Default)]
struct ResolvedFields {
    fields: Vec<hir::Field>,
    spans: Vec<Span>,
    calling: Vec<FnParamCalling>,
}
