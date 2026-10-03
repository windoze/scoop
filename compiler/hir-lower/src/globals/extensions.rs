//! Extension property declaration and signature checking.

use super::*;

impl Lowerer {
    pub(super) fn declare_extension_property(
        &mut self,
        declaration: &ast::PropertyDecl,
        file_index: usize,
        access: hir::DeclarationAccess,
        import_source: crate::imports::PropertyImportSource,
    ) {
        if declaration.modifier != ast::MethodModifier::Final || declaration.is_override {
            self.error(
                declaration.span,
                "extension properties cannot be open, abstract, or override".to_string(),
            );
            return;
        }
        match &declaration.body {
            ast::PropertyBodySyntax::Computed(_) | ast::PropertyBodySyntax::Delegated { .. } => {}
            _ => {
                self.error(
                    declaration.span,
                    format!(
                        "extension property `{}` must be computed or delegated",
                        declaration.name.text
                    ),
                );
                return;
            }
        }
        self.reject_logical_property_annotations("an extension property", &declaration.annotations);

        let mut type_params = Vec::new();
        for parameter in &declaration.type_params {
            if type_params
                .iter()
                .any(|existing: &hir::TypeParamDecl| existing.name == parameter.name.text)
            {
                self.error(
                    parameter.span,
                    format!("duplicate type parameter `{}`", parameter.name.text),
                );
                continue;
            }
            let id = self.fresh_type_param(type_params.len());
            type_params.push(crate::lower_type_param_decl(parameter, id));
        }
        type_params = self.resolve_type_parameter_constraints(
            type_params,
            0,
            &declaration.type_params,
            declaration.where_clause.as_ref(),
            "extension property",
        );
        self.type_params_in_scope = type_params.clone();
        let receiver_ty = declaration
            .receiver_ty
            .as_ref()
            .and_then(|receiver| self.resolve_type_ref(receiver));
        let property_ty = self.resolve_type_ref(&declaration.ty);
        self.type_params_in_scope.clear();
        let (Some(receiver_ty), Some(property_ty)) = (receiver_ty, property_ty) else {
            return;
        };

        let expected_property = self.next_property_id();
        let expected_extension =
            hir::ExtensionPropertyId::from_raw((self.extension_properties.len() as u32).into());
        let extension = self.extension_properties.alloc(hir::ExtensionProperty {
            property: expected_property,
            receiver_ty,
            type_params,
        });
        assert_eq!(extension, expected_extension);
        let (property, capability) =
            if matches!(declaration.body, ast::PropertyBodySyntax::Delegated { .. }) {
                if self.extension_properties[extension].type_params.is_empty() {
                    self.allocate_runtime_extension_delegate(
                        declaration,
                        file_index,
                        access,
                        extension,
                        property_ty,
                    )
                } else {
                    self.allocate_generic_extension_delegate(
                        declaration,
                        file_index,
                        access,
                        extension,
                        property_ty,
                    )
                }
            } else {
                let Some(capability) = self.allocate_property_accessors(
                    expected_property,
                    hir::PropertyOwner::Extension(extension),
                    access.clone(),
                    declaration,
                    None,
                    hir::MethodModifier::Final,
                ) else {
                    return;
                };
                let property = self.properties.alloc(hir::Property {
                    owner: hir::PropertyOwner::Extension(extension),
                    name: declaration.name.text.clone(),
                    access,
                    modifier: hir::MethodModifier::Final,
                    is_override: false,
                    overrides: Vec::new(),
                    ty: property_ty,
                    capability,
                    representation: hir::PropertyRepresentation::AccessorOnly,
                    span: declaration.span,
                });
                assert_eq!(property, expected_property);
                (property, capability)
            };
        self.top_level_namespaces.register_property(
            file_index,
            declaration.name.text.clone(),
            property,
            true,
        );
        self.property_files.insert(property, file_index);
        self.imports.bind_property(import_source, property);
        let getter = match self.property_getters[capability.getter()].implementation {
            hir::PropertyAccessorImplementation::Body(function) => function,
            hir::PropertyAccessorImplementation::Storage
            | hir::PropertyAccessorImplementation::Constant
            | hir::PropertyAccessorImplementation::AbstractSlot(_) => {
                unreachable!("a computed extension property has a getter body")
            }
        };
        assert!(
            self.extension_property_by_getter
                .insert(getter, property)
                .is_none(),
            "an accessor function belongs to one logical property"
        );
    }

    pub(crate) fn check_extension_property_signatures(&mut self) {
        let mut property_groups = self.top_level_namespaces.all_extension_property_groups();
        property_groups.sort_by_key(|properties| {
            properties
                .first()
                .expect("an extension-property name group is non-empty")
                .into_raw()
                .into_u32()
        });
        for properties in property_groups {
            for (index, property) in properties.iter().copied().enumerate() {
                let declaration = self.properties[property].clone();
                let hir::PropertyOwner::Extension(extension) = declaration.owner else {
                    unreachable!("the extension-property index contains only extensions")
                };
                let template = self.extension_properties[extension].clone();
                let getter =
                    match self.property_getters[declaration.capability.getter()].implementation {
                        hir::PropertyAccessorImplementation::Body(function) => function,
                        _ => unreachable!("extension properties have concrete getter bodies"),
                    };
                let mut receiver_bound = vec![false; self.signatures[&getter].type_params.len()];
                self.mark_type_params(self.extension_receivers[&getter], &mut receiver_bound);
                for (parameter, bound) in template.type_params.iter().zip(receiver_bound) {
                    if !bound {
                        self.current_file = self.property_files[&property];
                        self.error(
                            parameter.span,
                            format!(
                                "type parameter `{}` of extension property `{}` cannot be inferred solely from its receiver",
                                parameter.name, declaration.name
                            ),
                        );
                    }
                }

                let duplicate = properties[..index].iter().copied().any(|other| {
                    let other_declaration = &self.properties[other];
                    let other_getter = match self.property_getters
                        [other_declaration.capability.getter()]
                    .implementation
                    {
                        hir::PropertyAccessorImplementation::Body(function) => function,
                        _ => unreachable!("extension properties have concrete getter bodies"),
                    };
                    self.same_parameter_signature(getter, other_getter)
                        && (declaration.access.declared != hir::DeclaredVisibility::Private
                            || other_declaration.access.declared
                                != hir::DeclaredVisibility::Private
                            || self.property_files[&property] == self.property_files[&other])
                });
                if duplicate {
                    self.current_file = self.property_files[&property];
                    self.error(
                        declaration.span,
                        format!(
                            "extension property `{}` is already declared for the same receiver type",
                            declaration.name
                        ),
                    );
                }
            }
        }
    }
}
