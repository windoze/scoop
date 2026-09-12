use scoop_ast as ast;
use scoop_hir as hir;

use super::{PendingOrdinary, PendingRuntimeInitializer, PendingRuntimeInitializerKind};
use crate::Lowerer;

struct RuntimeDelegateRequest<'a> {
    declaration: &'a ast::PropertyDecl,
    file: usize,
    access: hir::DeclarationAccess,
    owner: hir::PropertyOwner,
    property_ty: hir::TypeId,
}

impl Lowerer {
    pub(super) fn allocate_runtime_top_level_delegate(
        &mut self,
        declaration: &PendingOrdinary<'_>,
    ) {
        let (property, _) = self.allocate_runtime_delegate(RuntimeDelegateRequest {
            declaration: declaration.declaration,
            file: declaration.file,
            access: declaration.access.clone(),
            owner: hir::PropertyOwner::TopLevel,
            property_ty: declaration.ty,
        });
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

    pub(super) fn allocate_runtime_extension_delegate(
        &mut self,
        declaration: &ast::PropertyDecl,
        file: usize,
        access: hir::DeclarationAccess,
        extension: hir::ExtensionPropertyId,
        property_ty: hir::TypeId,
    ) -> (hir::PropertyId, hir::PropertyCapability) {
        self.allocate_runtime_delegate(RuntimeDelegateRequest {
            declaration,
            file,
            access,
            owner: hir::PropertyOwner::Extension(extension),
            property_ty,
        })
    }

    fn allocate_runtime_delegate(
        &mut self,
        request: RuntimeDelegateRequest<'_>,
    ) -> (hir::PropertyId, hir::PropertyCapability) {
        let RuntimeDelegateRequest {
            declaration,
            file,
            access,
            owner,
            property_ty,
        } = request;
        let ast::PropertyBodySyntax::Delegated { expression, .. } = &declaration.body else {
            unreachable!("a runtime delegate owns delegated syntax")
        };
        let expected_property = self.next_property_id();
        let expected_global = hir::GlobalId::from_raw((self.globals.len() as u32).into());
        let expected_delegate =
            hir::DelegateStorageId::from_raw((self.delegate_storages.len() as u32).into());
        let expected_unit =
            hir::InitializationUnitId::from_raw((self.initialization_units.len() as u32).into());
        let failure_root =
            self.initialization_failure_roots
                .alloc(hir::InitializationFailureRoot {
                    unit: expected_unit,
                });
        let (initializer, ensure) =
            self.allocate_initialization_functions(expected_unit, declaration.span, file);
        let display_name =
            self.initialization_property_display_name(file, owner, &access, &declaration.name.text);
        let unit = self.initialization_units.alloc(hir::InitializationUnit {
            display_name: display_name.clone(),
            schedule: hir::InitializationSchedule::EagerStartup,
            kind: hir::InitializationUnitKind::EagerTopLevel {
                property: expected_property,
                storage: expected_global,
            },
            initializer,
            ensure,
            failure_root,
            dependencies: Vec::new(),
            span: declaration.span,
        });
        assert_eq!(unit, expected_unit);
        let capability = self
            .allocate_property_accessors(
                expected_property,
                owner,
                access.clone(),
                declaration,
                None,
                hir::MethodModifier::Final,
            )
            .expect("delegated properties always allocate generated accessors");
        self.mark_delegate_accessors_runtime_initialized(capability, unit);
        let global = self.globals.alloc(hir::Global {
            name: format!("$delegate${display_name}"),
            property: expected_property,
            // The effective type is committed before a successful Export HIR
            // can be emitted; diagnostics discard the in-progress graph.
            ty: self.unit,
            mutable: false,
            storage: hir::GlobalStorage::Managed {
                state: hir::HirStaticInitialState::ZeroedForRuntimeUnit { unit },
            },
            span: declaration.span,
        });
        assert_eq!(global, expected_global);
        let delegate_storage = self.delegate_storages.alloc(hir::DelegateStorage {
            property: expected_property,
            ty: self.unit,
            location: hir::DelegateStorageLocation::ManagedGlobal(global),
        });
        assert_eq!(delegate_storage, expected_delegate);
        let property = self.properties.alloc(hir::Property {
            owner,
            name: declaration.name.text.clone(),
            access,
            modifier: hir::MethodModifier::Final,
            is_override: false,
            overrides: Vec::new(),
            override_access: Vec::new(),
            ty: property_ty,
            capability,
            representation: hir::PropertyRepresentation::Delegated {
                storage: delegate_storage,
            },
            span: declaration.span,
        });
        assert_eq!(property, expected_property);
        self.pending_runtime_initializers
            .push(PendingRuntimeInitializer {
                unit,
                function: initializer,
                storage: global,
                file,
                span: declaration.span,
                kind: PendingRuntimeInitializerKind::Delegated {
                    property,
                    delegate_storage,
                    expression: (**expression).clone(),
                },
            });
        (property, capability)
    }

    fn mark_delegate_accessors_runtime_initialized(
        &mut self,
        capability: hir::PropertyCapability,
        unit: hir::InitializationUnitId,
    ) {
        let accessor_function = |implementation| match implementation {
            hir::PropertyAccessorImplementation::Body(function) => function,
            _ => unreachable!("delegated accessors are generated concrete bodies"),
        };
        let getter = accessor_function(self.property_getters[capability.getter()].implementation);
        self.runtime_accessor_units.insert(getter, unit);
        if let Some(setter) = capability.setter() {
            let setter = accessor_function(self.property_setters[setter].implementation);
            self.runtime_accessor_units.insert(setter, unit);
        }
    }
}
