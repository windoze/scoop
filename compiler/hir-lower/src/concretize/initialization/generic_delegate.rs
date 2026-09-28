//! One managed storage and initialization unit for each receiver application.

use super::*;

impl Concretizer<'_> {
    pub(in crate::concretize) fn request_generic_delegate(
        &mut self,
        reference: &export::GenericDelegateReference,
        substitution: &[concrete::TypeId],
    ) -> concrete::GenericDelegateStorageSpecializationId {
        let (property, source, effective_type, name, span) = match reference.template {
            export::GenericDelegateTemplateSource::Defined(template) => {
                let template = &self.source.generic_delegate_templates[template];
                let declaration = &self.source.properties[template.property];
                (
                    self.source.property_identities[template.property]
                        .extension_id()
                        .expect("a generic delegate is an extension property"),
                    InitializationSource::Defined(template.initialization),
                    template.ty,
                    declaration.name.clone(),
                    declaration.span,
                )
            }
            export::GenericDelegateTemplateSource::Imported(template) => {
                let declaration = &self.source.imported_generic_delegate_templates[template];
                (
                    declaration.property,
                    InitializationSource::ImportedDelegate(template),
                    declaration.effective_type,
                    declaration.diagnostic_path.clone(),
                    self.source.imported_generic_templates[declaration.initializer].span,
                )
            }
        };
        let arguments = reference
            .arguments
            .iter()
            .map(|argument| self.lower_type(*argument, substitution))
            .collect::<Vec<_>>();
        let key = (property, arguments);
        if let Some(id) = self.generic_delegate_by_key.get(&key) {
            return *id;
        }
        let initialization = self.request_initialization(InitializationKey {
            source,
            arguments: key.1.clone(),
        });
        let ty = self.lower_type(effective_type, &key.1);
        let id = concrete::GenericDelegateStorageSpecializationId::from_raw(
            u32::try_from(self.generic_delegate_specializations.len())
                .expect("delegate specializations fit their typed id domain")
                .into(),
        );
        let storage = self.globals.alloc(concrete::Global {
            name: format!("{name}$delegate"),
            storage_owner: concrete::PropertyStorageOwner::GenericDelegate(id),
            ty,
            mutable: false,
            storage: concrete::GlobalStorage::Managed {
                state: concrete::HirStaticInitialState::ZeroedForRuntimeUnit {
                    unit: initialization,
                },
            },
            span,
        });
        assert_eq!(
            self.generic_delegate_specializations.alloc(
                concrete::GenericDelegateStorageSpecialization {
                    storage,
                    initialization,
                }
            ),
            id,
        );
        self.generic_delegate_by_key.insert(key, id);
        id
    }
}
