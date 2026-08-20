//! Source delegate templates acquire storage only after receiver substitution.

use super::*;

impl Lowerer {
    pub(in crate::globals) fn allocate_generic_extension_delegate(
        &mut self,
        declaration: &ast::PropertyDecl,
        file: usize,
        access: hir::DeclarationAccess,
        extension: hir::ExtensionPropertyId,
        property_ty: hir::TypeId,
    ) -> (hir::PropertyId, hir::PropertyCapability) {
        let ast::PropertyBodySyntax::Delegated { expression, .. } = &declaration.body else {
            unreachable!("a generic delegate owns delegated syntax")
        };
        let property = self.next_property_id();
        let template = hir::GenericDelegateTemplateId::from_raw(
            (self.generic_delegate_templates.len() as u32).into(),
        );
        let unit =
            hir::InitializationUnitId::from_raw((self.initialization_units.len() as u32).into());
        let failure_root = self
            .initialization_failure_roots
            .alloc(hir::InitializationFailureRoot { unit });
        let (initializer, ensure) =
            self.allocate_initialization_functions(unit, declaration.span, file);
        let parameters = self.extension_properties[extension].type_params.clone();
        for function in [initializer, ensure] {
            self.register_generic(function, parameters.clone());
            self.signatures
                .get_mut(&function)
                .expect("allocated initialization signature")
                .type_params = parameters.clone();
        }
        let owner = hir::PropertyOwner::Extension(extension);
        let display_name =
            self.initialization_property_display_name(file, owner, &access, &declaration.name.text);
        assert_eq!(
            self.initialization_units.alloc(hir::InitializationUnit {
                display_name,
                schedule: hir::InitializationSchedule::LazyAccess,
                kind: hir::InitializationUnitKind::GenericDelegatedExtension { property, template },
                initializer,
                ensure,
                failure_root,
                dependencies: Vec::new(),
                span: declaration.span,
            }),
            unit,
        );
        let capability = self
            .allocate_property_accessors(
                property,
                owner,
                access.clone(),
                declaration,
                None,
                hir::MethodModifier::Final,
            )
            .expect("delegated properties have generated accessors");
        self.mark_delegate_accessors_runtime_initialized(capability, unit);
        assert_eq!(
            self.generic_delegate_templates
                .alloc(hir::GenericDelegateTemplate {
                    property,
                    // The checked by/provide expression commits its effective type.
                    ty: self.unit,
                    initialization: unit,
                }),
            template,
        );
        assert_eq!(
            self.properties.alloc(hir::Property {
                owner,
                name: declaration.name.text.clone(),
                access,
                modifier: hir::MethodModifier::Final,
                is_override: false,
                overrides: Vec::new(),
                ty: property_ty,
                capability,
                representation: hir::PropertyRepresentation::GenericDelegated { template },
                span: declaration.span,
            }),
            property,
        );
        self.pending_runtime_initializers
            .push(PendingRuntimeInitializer {
                unit,
                function: initializer,
                file,
                span: declaration.span,
                kind: PendingRuntimeInitializerKind::Delegated {
                    property,
                    storage: PendingDelegateStorage::Generic(template),
                    expression: (**expression).clone(),
                },
            });
        (property, capability)
    }

    pub(crate) fn generic_delegate_reference(
        &mut self,
        template: hir::GenericDelegateTemplateId,
        function: hir::FunctionId,
    ) -> hir::GenericDelegateReference {
        let property = self.generic_delegate_templates[template].property;
        let hir::PropertyOwner::Extension(extension) = self.properties[property].owner else {
            unreachable!("generic delegate templates belong to extension properties")
        };
        let parameters = self.signatures[&function].type_params.clone();
        assert_eq!(
            parameters.len(),
            self.extension_properties[extension].type_params.len()
        );
        let arguments = parameters
            .iter()
            .map(|parameter| self.intern_type(hir::Type::Param(parameter.id)))
            .collect();
        hir::GenericDelegateReference {
            template: hir::GenericDelegateTemplateSource::Defined(template),
            arguments: hir::NonEmptyVec::from_vec(arguments)
                .expect("a generic delegate has at least one receiver parameter"),
        }
    }
}
