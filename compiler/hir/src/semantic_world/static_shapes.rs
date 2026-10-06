use super::*;

impl<'input> ImportedSemanticWorld<'input> {
    /// Static source queries include the provider's existing private support
    /// declarations without adding them to source name lookup.
    pub(crate) fn nominal_source_provider(
        &self,
        declaration: SourceNominalId,
    ) -> Option<ImportedProviderView<'_, 'input>> {
        self.providers
            .iter()
            .find(|provider| {
                provider
                    .interface()
                    .nominal_interfaces()
                    .declaration(declaration)
                    .is_some()
            })
            .map(|provider| ImportedProviderView { provider })
    }

    pub(crate) fn annotation_source(
        &self,
        annotation: scoop_identity::PersistentAnnotationId,
    ) -> Option<&'input scoop_identity::SourceDeclarationKey> {
        self.providers.iter().find_map(|provider| {
            provider
                .foundation()
                .canonical_for_semantic_authority()
                .annotation(annotation)
                .map(scoop_identity::CborIdentityRecord::key)
        })
    }
}

impl<'input> ImportedProviderView<'_, 'input> {
    pub(crate) fn source_interface(self) -> &'input crate::CrossConeHirInterfaceSectionV1 {
        self.provider.interface()
    }

    pub(crate) fn source_foundation(self) -> &'input crate::CanonicalHirFoundation {
        self.provider
            .foundation()
            .canonical_for_semantic_authority()
    }
}
