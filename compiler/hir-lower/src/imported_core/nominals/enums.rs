//! Decode each enum payload once in its declaration's binder domain.

use super::*;

impl Lowerer {
    pub(super) fn imported_enum_type(
        &mut self,
        declaration: Arc<hir::ImportedNominalDeclaration>,
        arguments: Vec<hir::TypeId>,
    ) -> Result<hir::TypeId, ImportedSignatureTypeError> {
        let owner = declaration.owner();
        if !self.loaded_enum_definitions.contains_key(&owner) {
            self.load_enum_definition(declaration)?;
        }
        let application = self.intern_enum_application(owner, arguments);
        Ok(self.enum_applications[application].canonical_type)
    }

    fn load_enum_definition(
        &mut self,
        declaration: Arc<hir::ImportedNominalDeclaration>,
    ) -> Result<(), ImportedSignatureTypeError> {
        let hir::NominalSourceShapeV1::Enum(shape) = declaration.interface.source_shape() else {
            return Err(ImportedSignatureTypeError::Structural);
        };
        let owner = declaration.owner();
        let source_span = declaration.origin.origin().span();
        let span = Span {
            start: u32::try_from(source_span.start_byte())
                .map_err(|_| ImportedSignatureTypeError::Structural)?,
            end: u32::try_from(source_span.end_byte())
                .map_err(|_| ImportedSignatureTypeError::Structural)?,
        };
        let binders = declaration.interface.type_parameters().binders();
        let mut bindings = ImportedTypeBindings::new();
        let mut type_params = Vec::new();
        let mut arguments = Vec::new();
        for (index, binder) in binders.iter().enumerate() {
            let id = self.fresh_type_param(index);
            let ty = self.intern_type(hir::Type::Param(id));
            bindings.insert(
                SignatureTypeKey::Binder {
                    depth: 0,
                    index: index as u32,
                },
                ty,
            );
            arguments.push(ty);
            type_params.push(hir::TypeParamDecl {
                id,
                name: binder.name().as_str().to_owned(),
                bounds: hir::TypeParamBounds::Unconstrained,
                span,
            });
        }
        let self_application = self.intern_enum_application(owner, arguments);
        let self_type = self.enum_applications[self_application].canonical_type;
        // Recursive signatures can refer to the identity while this builder
        // completes its payload and conformance. No partial definition escapes.
        self.loaded_enum_definitions.insert(
            owner,
            hir::LoadedEnumDefinition {
                context_contracts: Vec::new(),
                declaration: Arc::clone(&declaration),
                definition: hir::EnumDefinition {
                    gc_free_pointee_requirements: Self::decoded_nominal_pointee_requirements(
                        &declaration,
                        &type_params,
                    ),
                    no_gc: declaration
                        .interface
                        .declaration_details()
                        .instantiation_conditions()
                        .no_gc(),
                    self_application,
                    type_params: type_params.clone(),
                    variants: Vec::new(),
                    interfaces: Vec::new(),
                    interface_implementations: Vec::new(),
                },
            },
        );
        let context_contracts = self.imported_nominal_contexts(owner, &bindings)?;
        self.loaded_enum_definitions
            .get_mut(&owner)
            .expect("the nominal builder registered its identity")
            .context_contracts = context_contracts;
        for (parameter, binder) in type_params.iter_mut().zip(binders) {
            *parameter = self
                .resolve_imported_type_parameter(binder, parameter.id, &bindings, span)
                .map_err(|_| ImportedSignatureTypeError::Structural)?;
        }
        self.loaded_enum_definitions
            .get_mut(&owner)
            .expect("the enum builder registered its identity")
            .definition
            .type_params = type_params.clone();
        let variants = shape
            .variants()
            .iter()
            .zip(&declaration.variant_names)
            .map(|(variant, names)| {
                let fields = variant
                    .fields()
                    .iter()
                    .zip(&names.fields)
                    .map(|(field, name)| {
                        Ok(hir::Field {
                            name: name.clone(),
                            ty: self.imported_signature_type_with_bindings(
                                field.value_type(),
                                &bindings,
                            )?,
                        })
                    })
                    .collect::<Result<Vec<_>, ImportedSignatureTypeError>>()?;
                Ok(hir::Variant {
                    name: names.name.clone(),
                    style: match variant.style() {
                        hir::EnumSourceVariantStyleV1::Unit => hir::VariantStyle::Unit,
                        hir::EnumSourceVariantStyleV1::Positional => hir::VariantStyle::Positional,
                        hir::EnumSourceVariantStyleV1::Named => hir::VariantStyle::Named,
                        hir::EnumSourceVariantStyleV1::Constructor => {
                            hir::VariantStyle::Constructor
                        }
                    },
                    fields,
                })
            })
            .collect::<Result<Vec<_>, ImportedSignatureTypeError>>()?;
        self.loaded_enum_definitions
            .get_mut(&owner)
            .expect("the enum builder registered its identity")
            .definition
            .variants = variants;
        let interfaces = declaration
            .interface
            .exact_supertypes()
            .values()
            .iter()
            .map(|parent| self.imported_signature_type_with_bindings(parent, &bindings))
            .collect::<Result<Vec<_>, _>>()?;
        self.loaded_enum_definitions
            .get_mut(&owner)
            .expect("the enum builder registered its identity")
            .definition
            .interfaces = interfaces.clone();
        let scope_len = self.type_params_in_scope.len();
        self.type_params_in_scope.extend(type_params);
        let implementations =
            self.resolve_imported_interface_implementations(self_type, &declaration, &interfaces);
        self.type_params_in_scope.truncate(scope_len);
        let definition = &mut self
            .loaded_enum_definitions
            .get_mut(&owner)
            .expect("the enum builder registered its identity")
            .definition;
        definition.interface_implementations = implementations?;
        Ok(())
    }
}
