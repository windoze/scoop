//! Top-level property storage and initializer allocation.

use super::*;

impl Lowerer {
    pub(super) fn resolve_static_top_level_properties(
        &mut self,
        declarations: &[PendingOrdinary<'_>],
    ) {
        for declaration in declarations {
            self.current_file = declaration.file;
            let initializer = match &declaration.declaration.body {
                ast::PropertyBodySyntax::Initializer { expression, .. } => {
                    self.static_property_constant(expression, declaration.ty)
                }
                ast::PropertyBodySyntax::OptionalOmitted => {
                    if !declaration.declaration.mutable || self.as_option(declaration.ty).is_none()
                    {
                        self.error(
                            declaration.declaration.span,
                            format!(
                                "property `{}` without an initializer must be a mutable Option property",
                                declaration.declaration.name.text
                            ),
                        );
                        None
                    } else {
                        self.static_none_constant(declaration.ty)
                    }
                }
                ast::PropertyBodySyntax::Const(_)
                | ast::PropertyBodySyntax::Computed(_)
                | ast::PropertyBodySyntax::Abstract
                | ast::PropertyBodySyntax::ExternStorage => {
                    unreachable!(
                        "the ordinary property worklist contains only stored or delegated properties"
                    )
                }
                ast::PropertyBodySyntax::Delegated { .. } => {
                    self.allocate_runtime_top_level_delegate(declaration);
                    continue;
                }
            };
            if let Some(initializer) = initializer {
                self.allocate_image_top_level_property(declaration, initializer);
            } else if let Some(expression) = declaration.declaration.initializer() {
                self.allocate_runtime_top_level_property(declaration, expression.clone());
            }
        }
    }

    pub(super) fn allocate_image_top_level_property(
        &mut self,
        declaration: &PendingOrdinary<'_>,
        initializer: hir::HirConstantImage,
    ) {
        let expected_property = self.next_property_id();
        let expected_global = hir::GlobalId::from_raw((self.globals.len() as u32).into());
        let backing = hir::PropertyBacking::TopLevelGlobal {
            storage: expected_global,
            initialization: hir::TopLevelInitialization::Image,
        };
        let Some(capability) = self.allocate_property_accessors(
            expected_property,
            hir::PropertyOwner::TopLevel,
            declaration.access.clone(),
            declaration.declaration,
            Some(backing),
            hir::MethodModifier::Final,
        ) else {
            return;
        };
        let global = self.globals.alloc(hir::Global {
            name: declaration.declaration.name.text.clone(),
            property: expected_property,
            ty: declaration.ty,
            mutable: declaration.declaration.mutable,
            storage: hir::GlobalStorage::Managed {
                state: hir::HirStaticInitialState::EncodedStaticValue {
                    payload: initializer,
                },
            },
            span: declaration.declaration.span,
        });
        assert_eq!(global, expected_global);
        let property = self.properties.alloc(hir::Property {
            owner: hir::PropertyOwner::TopLevel,
            name: declaration.declaration.name.text.clone(),
            access: declaration.access.clone(),
            modifier: hir::MethodModifier::Final,
            is_override: false,
            overrides: Vec::new(),
            ty: declaration.ty,
            capability,
            representation: hir::PropertyRepresentation::Stored(hir::StoredProperty { backing }),
            span: declaration.declaration.span,
        });
        assert_eq!(property, expected_property);
        self.top_level_namespaces.register_property(
            declaration.file,
            declaration.declaration.name.text.clone(),
            property,
            false,
        );
        self.property_files.insert(property, declaration.file);
        self.imports
            .bind_property(declaration.import_source, property);
    }

    pub(super) fn allocate_runtime_top_level_property(
        &mut self,
        declaration: &PendingOrdinary<'_>,
        expression: ast::Expr,
    ) {
        let expected_property = self.next_property_id();
        let expected_global = hir::GlobalId::from_raw((self.globals.len() as u32).into());
        let expected_unit =
            hir::InitializationUnitId::from_raw((self.initialization_units.len() as u32).into());
        let failure_root =
            self.initialization_failure_roots
                .alloc(hir::InitializationFailureRoot {
                    unit: expected_unit,
                });
        let (initializer, ensure) = self.allocate_initialization_functions(
            expected_unit,
            declaration.declaration.span,
            declaration.file,
        );
        let display_name = self.initialization_property_display_name(
            declaration.file,
            hir::PropertyOwner::TopLevel,
            &declaration.access,
            &declaration.declaration.name.text,
        );
        let unit = self.initialization_units.alloc(hir::InitializationUnit {
            display_name,
            schedule: hir::InitializationSchedule::EagerStartup,
            kind: hir::InitializationUnitKind::EagerTopLevel {
                property: expected_property,
                storage: expected_global,
            },
            initializer,
            ensure,
            failure_root,
            dependencies: Vec::new(),
            span: declaration.declaration.span,
        });
        assert_eq!(unit, expected_unit);
        let backing = hir::PropertyBacking::TopLevelGlobal {
            storage: expected_global,
            initialization: hir::TopLevelInitialization::Runtime(unit),
        };
        let Some(capability) = self.allocate_property_accessors(
            expected_property,
            hir::PropertyOwner::TopLevel,
            declaration.access.clone(),
            declaration.declaration,
            Some(backing),
            hir::MethodModifier::Final,
        ) else {
            return;
        };
        let global = self.globals.alloc(hir::Global {
            name: declaration.declaration.name.text.clone(),
            property: expected_property,
            ty: declaration.ty,
            mutable: declaration.declaration.mutable,
            storage: hir::GlobalStorage::Managed {
                state: hir::HirStaticInitialState::ZeroedForRuntimeUnit { unit },
            },
            span: declaration.declaration.span,
        });
        assert_eq!(global, expected_global);
        let property = self.properties.alloc(hir::Property {
            owner: hir::PropertyOwner::TopLevel,
            name: declaration.declaration.name.text.clone(),
            access: declaration.access.clone(),
            modifier: hir::MethodModifier::Final,
            is_override: false,
            overrides: Vec::new(),
            ty: declaration.ty,
            capability,
            representation: hir::PropertyRepresentation::Stored(hir::StoredProperty { backing }),
            span: declaration.declaration.span,
        });
        assert_eq!(property, expected_property);
        self.top_level_namespaces.register_property(
            declaration.file,
            declaration.declaration.name.text.clone(),
            property,
            false,
        );
        self.property_files.insert(property, declaration.file);
        self.imports
            .bind_property(declaration.import_source, property);
        self.pending_runtime_initializers
            .push(PendingRuntimeInitializer {
                unit,
                function: initializer,
                file: declaration.file,
                span: declaration.declaration.span,
                kind: PendingRuntimeInitializerKind::Stored {
                    storage: global,
                    ty: declaration.ty,
                    expression,
                },
            });
    }
}
