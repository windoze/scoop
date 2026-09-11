//! Persistent LocalConcrete value locations retained by MIR lowering.

use std::collections::HashMap;

use scoop_hir::concrete as hir;
use scoop_mir as mir;

#[derive(Default)]
pub(super) struct SourceLocalValueRegistry {
    entries: Vec<mir::SourceLocalValueIdentity>,
}

impl SourceLocalValueRegistry {
    pub(super) fn record(
        &mut self,
        function: mir::FunctionId,
        local: mir::LocalId,
        identity: &hir::LocalValueIdentityRecord,
    ) {
        self.entries.push(mir::SourceLocalValueIdentity::new(
            function,
            local,
            identity.clone(),
        ));
    }

    /// Move the original body locations into the coroutine driver and retain
    /// source-parameter aliases in the replacement ABI wrapper. Body locals
    /// must not remain attached to the wrapper because its local arena is a
    /// different semantic domain even when raw indices happen to coincide.
    pub(super) fn remap_coroutine_function(
        &mut self,
        source: mir::FunctionId,
        driver: mir::FunctionId,
        wrapper_parameters: &HashMap<mir::LocalId, mir::LocalId>,
    ) {
        let mut remapped = Vec::with_capacity(self.entries.len() + wrapper_parameters.len());
        for entry in self.entries.drain(..) {
            if entry.function() != source {
                remapped.push(entry);
                continue;
            }
            if let Some(wrapper_local) = wrapper_parameters.get(&entry.local()) {
                remapped.push(mir::SourceLocalValueIdentity::new(
                    source,
                    *wrapper_local,
                    entry.identity_record().clone(),
                ));
            }
            remapped.push(mir::SourceLocalValueIdentity::new(
                driver,
                entry.local(),
                entry.identity_record().clone(),
            ));
        }
        self.entries = remapped;
    }

    pub(super) fn finish(self) -> mir::SourceLocalValueIdentities {
        mir::SourceLocalValueIdentities::checked(self.entries)
            .expect("MIR lowering records each source local location exactly once")
    }
}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        CallableMaterialization, CallableMaterializationContext, CallableTemplateOwner,
        CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope,
        DefinitionOwnerChain, LocalValueKey, LocalValueSelector, PackagePath, PersistentFunctionId,
        SourceDeclarationKey, SourceDeclarationSite,
    };

    use super::*;

    fn record(parameter: u32) -> hir::LocalValueIdentityRecord {
        let declaration = SourceDeclarationKey::function(
            SourceDeclarationSite::new(
                ConeIdentity::SINGLE_FILE,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("coroutineSource").unwrap(),
            0,
            None,
            Vec::new(),
        );
        CborIdentityRecord::from_key(LocalValueKey::new(
            CallableMaterialization::new(
                CallableTemplateOwner::Function(
                    PersistentFunctionId::from_source_declaration(&declaration).unwrap(),
                ),
                CallableMaterializationContext::NoSubstitution,
            ),
            LocalValueSelector::Parameter {
                declaration_index: parameter,
            },
        ))
        .unwrap()
    }

    #[test]
    fn coroutine_remap_moves_body_values_and_aliases_only_wrapper_parameters() {
        let source = mir::FunctionId::from_raw(2_u32.into());
        let driver = mir::FunctionId::from_raw(7_u32.into());
        let source_parameter = mir::LocalId::from_raw(0_u32.into());
        let source_body_local = mir::LocalId::from_raw(3_u32.into());
        let wrapper_parameter = mir::LocalId::from_raw(1_u32.into());
        let parameter_record = record(0);
        let body_record = record(1);
        let mut registry = SourceLocalValueRegistry::default();
        registry.record(source, source_parameter, &parameter_record);
        registry.record(source, source_body_local, &body_record);

        registry.remap_coroutine_function(
            source,
            driver,
            &HashMap::from([(source_parameter, wrapper_parameter)]),
        );
        let identities = registry.finish();

        assert_eq!(
            identities
                .get(source, wrapper_parameter)
                .unwrap()
                .identity_record(),
            &parameter_record
        );
        assert_eq!(
            identities
                .get(driver, source_parameter)
                .unwrap()
                .identity_record(),
            &parameter_record
        );
        assert_eq!(
            identities
                .get(driver, source_body_local)
                .unwrap()
                .identity_record(),
            &body_record
        );
        assert!(identities.get(source, source_body_local).is_none());
    }
}
