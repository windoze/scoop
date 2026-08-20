use super::*;

impl Concretizer<'_> {
    pub(super) fn initialization_identity(
        &self,
        key: &InitializationKey,
        exact_types: &concrete::ExactTypeIdentities,
    ) -> concrete::InitializationUnitIdentityRecord {
        if let InitializationSource::Defined(source) = key.source
            && let scoop_identity::InitializationUnitKey::GenericCompanionTemplate(companion) =
                self.source.initialization_unit_identities[source].key()
        {
            return scoop_identity::CborIdentityRecord::from_key(
                scoop_identity::InitializationUnitKey::GenericCompanionApplication {
                    companion: *companion,
                    arguments: scoop_identity::NonEmptyVec::new(
                        key.arguments
                            .iter()
                            .map(|ty| exact_types[*ty].id())
                            .collect(),
                    )
                    .expect("a generic companion has host arguments"),
                },
            )
            .expect("a companion application has an encodable initialization key");
        }
        let property = match key.source {
            InitializationSource::ImportedCompanion(template) => {
                let export::SourceNominalId::GenericTemplate(companion) =
                    self.source.imported_companion_templates[template]
                        .declaration
                        .owner()
                else {
                    unreachable!("imported companion templates have generic owners")
                };
                return scoop_identity::CborIdentityRecord::from_key(
                    scoop_identity::InitializationUnitKey::GenericCompanionApplication {
                        companion,
                        arguments: scoop_identity::NonEmptyVec::new(
                            key.arguments
                                .iter()
                                .map(|ty| exact_types[*ty].id())
                                .collect(),
                        )
                        .expect("generic companions have host arguments"),
                    },
                )
                .expect("a companion application has an encodable initialization key");
            }
            InitializationSource::ImportedDelegate(template) => {
                self.source.imported_generic_delegate_templates[template].property
            }
            InitializationSource::Defined(source) => {
                let export::InitializationUnitKind::GenericDelegatedExtension { property, .. } =
                    self.source.initialization_units[source].kind
                else {
                    assert!(key.arguments.is_empty());
                    return self.source.initialization_unit_identities[source].clone();
                };
                self.source.property_identities[property]
                    .extension_id()
                    .expect("a generic delegate is an extension property")
            }
        };
        let receiver_arguments = scoop_identity::NonEmptyVec::new(
            key.arguments
                .iter()
                .map(|argument| exact_types[*argument].id())
                .collect(),
        )
        .expect("a generic delegate retains nonempty receiver arguments");
        scoop_identity::CborIdentityRecord::from_key(
            scoop_identity::InitializationUnitKey::GenericDelegatedExtensionApplication {
                property,
                receiver_arguments,
            },
        )
        .expect("a delegate application has a canonical initialization identity")
    }
}
