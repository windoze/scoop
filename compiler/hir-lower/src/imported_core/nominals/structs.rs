//! Decode each struct's fields once in its declaration's binder domain.

use super::*;

impl Lowerer {
    pub(super) fn imported_struct_type(
        &mut self,
        declaration: Arc<hir::ImportedNominalDeclaration>,
        arguments: Vec<hir::TypeId>,
    ) -> Result<hir::TypeId, ImportedSignatureTypeError> {
        let owner = declaration.owner();
        if !self.loaded_struct_definitions.contains_key(&owner) {
            self.load_struct_definition(declaration)?;
        }
        let application = self.intern_struct_application(owner, arguments);
        Ok(self.struct_applications[application].canonical_type)
    }

    fn load_struct_definition(
        &mut self,
        mut declaration: Arc<hir::ImportedNominalDeclaration>,
    ) -> Result<(), ImportedSignatureTypeError> {
        self.normalize_imported_struct_c_abi(&mut declaration)?;
        let (c_layout, interior_mutable, representation) =
            match declaration.interface.source_shape() {
                hir::NominalSourceShapeV1::Struct(shape) => (
                    match shape.c_layout_policy() {
                        hir::NominalCLayoutPolicyV1::Ordinary => None,
                        hir::NominalCLayoutPolicyV1::CLayout { contract } => Some(contract),
                    },
                    shape.interior_mutable(),
                    hir::StructRepresentation::Declared(Vec::new()),
                ),
                hir::NominalSourceShapeV1::Intrinsic(shape)
                    if shape.family() == hir::IntrinsicTypeKind::FunPtr =>
                {
                    (
                        None,
                        false,
                        hir::StructRepresentation::Intrinsic(shape.family()),
                    )
                }
                _ => return Err(ImportedSignatureTypeError::Structural),
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
        let self_application = self.intern_struct_application(owner, arguments);
        let self_type = self.struct_applications[self_application].canonical_type;
        // Register the identity before resolving recursive fields and bounds.
        // The builder completes the definition before any HIR can be emitted.
        self.loaded_struct_definitions.insert(
            owner,
            hir::LoadedStructDefinition {
                declaration: Arc::clone(&declaration),
                definition: hir::StructDefinition {
                    gc_free_pointee_requirements: Self::decoded_nominal_pointee_requirements(
                        &declaration,
                        &type_params,
                    ),
                    attributes: hir::StructAttributes {
                        no_gc: declaration
                            .interface
                            .declaration_details()
                            .instantiation_conditions()
                            .no_gc(),
                        c_layout,
                        interior_mutable,
                    },
                    self_application,
                    type_params: type_params.clone(),
                    representation,
                    interfaces: Vec::new(),
                    interface_implementations: Vec::new(),
                },
            },
        );
        for (parameter, binder) in type_params.iter_mut().zip(binders) {
            *parameter = self
                .resolve_imported_type_parameter(binder, parameter.id, &bindings, span)
                .map_err(|_| ImportedSignatureTypeError::Structural)?;
        }
        self.loaded_struct_definitions
            .get_mut(&owner)
            .expect("the struct builder registered its identity")
            .definition
            .type_params = type_params.clone();
        if let hir::NominalSourceShapeV1::Struct(shape) = declaration.interface.source_shape() {
            let fields = shape
                .fields()
                .iter()
                .zip(&declaration.field_sources)
                .map(|(field, source)| {
                    Ok(hir::Field {
                        name: source.name.clone(),
                        ty: self
                            .imported_signature_type_with_bindings(field.value_type(), &bindings)?,
                    })
                })
                .collect::<Result<Vec<_>, ImportedSignatureTypeError>>()?;
            self.loaded_struct_definitions
                .get_mut(&owner)
                .expect("the struct builder registered its identity")
                .definition
                .representation = hir::StructRepresentation::Declared(fields);
        }
        let interfaces = declaration
            .interface
            .exact_supertypes()
            .values()
            .iter()
            .map(|parent| self.imported_signature_type_with_bindings(parent, &bindings))
            .collect::<Result<Vec<_>, _>>()?;
        self.loaded_struct_definitions
            .get_mut(&owner)
            .expect("the struct builder registered its identity")
            .definition
            .interfaces = interfaces.clone();
        let scope_len = self.type_params_in_scope.len();
        self.type_params_in_scope.extend(type_params);
        let implementations =
            self.resolve_imported_interface_implementations(self_type, &declaration, &interfaces);
        self.type_params_in_scope.truncate(scope_len);
        self.loaded_struct_definitions
            .get_mut(&owner)
            .expect("the struct builder registered its identity")
            .definition
            .interface_implementations = implementations?;
        Ok(())
    }

    fn normalize_imported_struct_c_abi(
        &self,
        declaration: &mut Arc<hir::ImportedNominalDeclaration>,
    ) -> Result<(), ImportedSignatureTypeError> {
        let CoreLoweringAuthority::Imported(core) = &self.core else {
            return Ok(());
        };
        let handles = [core.ffi().pinned_ptr(), core.ffi().gc_handle()];
        if !handles.iter().any(|handle| {
            declaration.owner() == hir::SourceNominalId::GenericTemplate(handle.persistent())
        }) {
            return Ok(());
        }
        let [field] = declaration.interface.source_shape().declared_fields() else {
            return Err(ImportedSignatureTypeError::Structural);
        };
        let projection = hir::NativeBoundaryCAbiV1::UInt64Field {
            field: field.field(),
        };
        if declaration.c_abi == projection {
            return Ok(());
        }
        if declaration.c_abi != hir::NativeBoundaryCAbiV1::SourceRepresentation {
            return Err(ImportedSignatureTypeError::Structural);
        }
        Arc::make_mut(declaration).c_abi = projection;
        Ok(())
    }
}
