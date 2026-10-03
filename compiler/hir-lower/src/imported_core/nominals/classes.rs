//! Decode each class once in its declaration's binder domain.

use super::*;

impl Lowerer {
    pub(super) fn imported_class_type(
        &mut self,
        declaration: Arc<hir::ImportedNominalDeclaration>,
        arguments: Vec<hir::TypeId>,
    ) -> Result<hir::TypeId, ImportedSignatureTypeError> {
        let owner = declaration.owner();
        if !self.loaded_class_definitions.contains_key(&owner) {
            self.load_class_definition(declaration)?;
        }
        let application = self.intern_class_application(owner, arguments);
        Ok(self.class_applications[application].canonical_type)
    }

    fn load_class_definition(
        &mut self,
        declaration: Arc<hir::ImportedNominalDeclaration>,
    ) -> Result<(), ImportedSignatureTypeError> {
        let representation = match declaration.interface.source_shape() {
            hir::NominalSourceShapeV1::Class(_) | hir::NominalSourceShapeV1::Object(_) => {
                hir::ClassRepresentation::Declared
            }
            hir::NominalSourceShapeV1::Intrinsic(value) => {
                hir::ClassRepresentation::Intrinsic(value.family())
            }
            _ => return Err(ImportedSignatureTypeError::Structural),
        };
        let modifier = match declaration.interface.declaration_details().modality() {
            hir::NominalInheritanceModalityV1::Final => hir::ClassModifier::Final,
            hir::NominalInheritanceModalityV1::Open => hir::ClassModifier::Open,
            hir::NominalInheritanceModalityV1::Abstract => hir::ClassModifier::Abstract,
            hir::NominalInheritanceModalityV1::Interface => {
                return Err(ImportedSignatureTypeError::Structural);
            }
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
        let self_application = self.intern_class_application(owner, arguments);
        let self_type = self.class_applications[self_application].canonical_type;
        // Recursive references share this identity while the builder completes
        // the declaration. No incomplete definition leaves HIR lowering.
        self.loaded_class_definitions.insert(
            owner,
            hir::LoadedClassDefinition {
                declaration: Arc::clone(&declaration),
                definition: hir::ClassDefinition {
                    release_policy: Default::default(),
                    gc_free_pointee_requirements: Self::decoded_nominal_pointee_requirements(
                        &declaration,
                        &type_params,
                    ),
                    modifier,
                    self_application,
                    type_params: type_params.clone(),
                    representation,
                    fields: Vec::new(),
                    base_class: None,
                    interfaces: Vec::new(),
                    interface_implementations: Vec::new(),
                },
                virtual_methods: Vec::new(),
            },
        );
        for (parameter, binder) in type_params.iter_mut().zip(binders) {
            *parameter = self
                .resolve_imported_type_parameter(binder, parameter.id, &bindings, span)
                .map_err(|_| ImportedSignatureTypeError::Structural)?;
        }
        self.loaded_class_definitions
            .get_mut(&owner)
            .expect("the class builder registered its identity")
            .definition
            .type_params = type_params.clone();
        let mut base_class = None;
        let mut interfaces = Vec::new();
        for parent in declaration.interface.exact_supertypes().values() {
            let parent = self.imported_signature_type_with_bindings(parent, &bindings)?;
            match &self.types[parent] {
                hir::Type::Class(_) => base_class = Some(parent),
                hir::Type::Interface(_) => interfaces.push(parent),
                _ => return Err(ImportedSignatureTypeError::Structural),
            }
        }
        let fields = declaration
            .interface
            .source_shape()
            .declared_fields()
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
        let definition = &mut self
            .loaded_class_definitions
            .get_mut(&owner)
            .expect("the class builder registered its identity")
            .definition;
        definition.fields = fields;
        definition.base_class = base_class;
        definition.interfaces = interfaces;
        definition.release_policy =
            match declaration.interface.declaration_details().release_policy() {
                hir::NominalReleasePolicyV1::None => hir::ReleasePolicy::None,
                hir::NominalReleasePolicyV1::SynchronousGcFree { requirements } => {
                    hir::ReleasePolicy::SynchronousGcFree {
                        hook: hir::ExportReleaseHookRef::Imported {
                            requirements: requirements
                                .iter()
                                .map(|binder| type_params[binder.index as usize].id)
                                .collect(),
                        },
                    }
                }
            };
        let scope_len = self.type_params_in_scope.len();
        self.type_params_in_scope.extend(type_params);
        let dispatch = self.resolve_imported_class_dispatch(self_type, &declaration);
        self.type_params_in_scope.truncate(scope_len);
        let (virtual_methods, implementations) = dispatch?;
        let loaded = self
            .loaded_class_definitions
            .get_mut(&owner)
            .expect("the class builder registered its identity");
        loaded.virtual_methods = virtual_methods;
        loaded.definition.interface_implementations = implementations;
        Ok(())
    }
}
