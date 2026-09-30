//! Import the checked initializer and synthesize its protocol-owned ensure entry.

use super::*;
use scoop_identity::{
    GeneratedCallableKey, InitializationCallableRole, InitializationUnitKey,
    PersistentExtensionPropertyId, PersistentGeneratedCallableId, PersistentInitializationUnitId,
};

impl Lowerer {
    pub(crate) fn request_imported_generic_delegate(
        &mut self,
        property: PersistentExtensionPropertyId,
    ) -> Result<hir::ImportedGenericDelegateTemplateId, String> {
        if let Some(id) = self.imported_generic_templates.delegates.get(&property) {
            return Ok(*id);
        }
        let dependencies = self
            .dependencies
            .as_ref()
            .ok_or("delegate has no provider catalog")?;
        let delegate = dependencies
            .generic_delegate(property)
            .ok_or("dependency property is missing its delegate template")?;
        let declaration = dependencies
            .property_declaration(scoop_identity::PropertyOwner::ExtensionProperty(property))
            .ok_or("dependency delegate is missing its property declaration")?;
        if declaration.type_parameters().binders().len()
            != delegate.initializer().type_parameters().arguments().len()
        {
            return Err("delegate initializer binders differ from its property declaration".into());
        }
        let body = dependencies
            .callable_body(delegate.initializer().owner())
            .ok_or("dependency delegate is missing its initializer body")?;
        let source = PreparedImportedCallableSource::Body(body);
        let body = source.body();
        let origin = self.import_generic_definition(&source, body.definition_origin())?;
        let mut bindings = ImportedTypeBindings::new();
        let mut parameters = Vec::new();
        for (slot, key) in body.type_parameters().arguments().iter().enumerate() {
            let parameter = self.fresh_type_param(slot);
            let ty = self.intern_type(hir::Type::Param(parameter));
            bindings.insert(key.clone(), ty);
            parameters.push(parameter);
        }
        let effective_type = self.imported_generic_type(delegate.effective_type(), &bindings)?;
        let return_type = self.imported_generic_type(body.result(), &bindings)?;
        if return_type != self.unit {
            return Err("delegate initializer must return Unit".into());
        }
        let (locals, _) = self.imported_body_locals(&source, &bindings)?;
        let (no_gc_type_params, gc_free_pointee_requirements) =
            self.imported_body_predicates(body, &bindings)?;
        let template = hir::ImportedGenericDelegateTemplateId::from_raw(
            u32::try_from(self.imported_generic_delegate_templates.len())
                .expect("imported delegate templates fit their typed id domain")
                .into(),
        );
        let unit = PersistentInitializationUnitId::from_key(
            &InitializationUnitKey::ExtensionProperty(property),
        )
        .map_err(|error| error.to_string())?;
        let owner = |role| {
            PersistentGeneratedCallableId::from_key(&GeneratedCallableKey::Initialization {
                unit,
                role,
            })
            .map_err(|error| error.to_string())
        };
        let initializer_signature = hir::ImportedGenericCallableSignature {
            declaration: hir::ImportedCallableTemplateOrigin::Initialization {
                template,
                owner: owner(InitializationCallableRole::Initializer)?,
            },
            signature: hir::CallableSignature::from_source_effects(
                format!("{}$initialize", delegate.diagnostic_path()),
                Vec::new(),
                return_type,
                body.effects(),
                origin.span,
            ),
            type_parameters: hir::ImportedCallableTypeParameters::Substitution(parameters),
            no_gc_type_params,
            gc_free_pointee_requirements,
            receiver: None,
            origin,
        };
        let mut ensure_signature = initializer_signature.clone();
        ensure_signature.declaration = hir::ImportedCallableTemplateOrigin::Initialization {
            template,
            owner: owner(InitializationCallableRole::Ensure)?,
        };
        ensure_signature.name = format!("{}$ensure", delegate.diagnostic_path());
        let initializer = self.allocate_imported_template(PreparedImportedGeneric {
            signature: initializer_signature,
            source,
            bindings,
            locals,
            statements: None,
        });
        let ensure = self.allocate_imported_template(PreparedImportedGeneric {
            signature: ensure_signature,
            source: PreparedImportedCallableSource::InitializationEnsure,
            bindings: ImportedTypeBindings::new(),
            locals: Arena::new(),
            statements: None,
        });
        assert_eq!(
            self.imported_generic_delegate_templates
                .alloc(hir::ImportedGenericDelegateTemplate {
                    property,
                    effective_type,
                    initializer,
                    ensure,
                    diagnostic_path: delegate.diagnostic_path().to_owned(),
                },),
            template
        );
        self.imported_generic_templates
            .delegates
            .insert(property, template);
        Ok(template)
    }
}
