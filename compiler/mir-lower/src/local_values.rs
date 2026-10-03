//! Persistent semantic value locations retained by MIR lowering.

use std::collections::HashMap;

use scoop_hir::concrete as hir;
use scoop_mir as mir;

use crate::cfg::GeneratedLocalValue;

#[derive(Default)]
pub(super) struct LocalValueRegistry {
    entries: Vec<mir::LocalValueIdentity>,
    next_ordinals: HashMap<
        (
            hir::CallableMaterialization,
            hir::StructuralDefinitionSiteRole,
        ),
        u32,
    >,
}

impl LocalValueRegistry {
    pub(super) fn record(
        &mut self,
        function: impl Into<mir::LocalValueOwner>,
        local: mir::LocalId,
        identity: &hir::LocalValueIdentityRecord,
    ) {
        self.observe_selector(identity.key().owner(), identity.key().selector());
        self.entries.push(mir::LocalValueIdentity::for_owner(
            function.into(),
            local,
            mir::LocalValueIdentityAuthority::Hir,
            identity.clone(),
        ));
    }

    pub(super) fn get(
        &self,
        function: mir::FunctionId,
        local: mir::LocalId,
    ) -> Option<&mir::LocalValueIdentityRecord> {
        self.entries
            .iter()
            .find(|entry| entry.owner() == function.into() && entry.local() == local)
            .map(mir::LocalValueIdentity::identity_record)
    }

    pub(super) fn record_generated(
        &mut self,
        function: impl Into<mir::LocalValueOwner>,
        owner: hir::CallableMaterialization,
        values: &[GeneratedLocalValue],
    ) {
        let function = function.into();
        for value in values {
            self.record_generated_local(function, value.local, owner, value.site_role, value.role);
        }
    }

    /// Record the receiver and declared arguments of a generated dispatch
    /// callable. The physical receiver is the first MIR parameter and does
    /// not consume a source-signature parameter index.
    pub(super) fn record_generated_dispatch_parameters(
        &mut self,
        function: mir::FunctionId,
        owner: hir::CallableMaterialization,
        params: &[mir::Param],
    ) {
        for (index, param) in params.iter().enumerate() {
            let selector = if index == 0 {
                hir::LocalValueSelector::This
            } else {
                hir::LocalValueSelector::Parameter {
                    declaration_index: u32::try_from(index - 1)
                        .expect("one generated callable cannot exhaust parameter indices"),
                }
            };
            let identity =
                hir::CborIdentityRecord::from_key(hir::LocalValueKey::new(owner, selector))
                    .expect("generated callable parameters have hashable identities");
            self.entries.push(mir::LocalValueIdentity::from_mir(
                function,
                param.local,
                identity,
            ));
        }
    }

    pub(super) fn record_generated_local(
        &mut self,
        function: impl Into<mir::LocalValueOwner>,
        local: mir::LocalId,
        owner: hir::CallableMaterialization,
        site_role: hir::StructuralDefinitionSiteRole,
        role: hir::SyntheticLocalRole,
    ) {
        let ordinal = self.next_ordinals.entry((owner, site_role)).or_default();
        let path = hir::StructuralDefinitionPath::from_first(
            hir::StructuralPathSegment::new(site_role, *ordinal),
            [],
        );
        *ordinal = ordinal
            .checked_add(1)
            .expect("one callable cannot exhaust generated local ordinals");
        let identity = hir::CborIdentityRecord::from_key(hir::LocalValueKey::new(
            owner,
            hir::LocalValueSelector::Synthetic { path, role },
        ))
        .expect("generated MIR local selectors have hashable identities");
        self.entries.push(mir::LocalValueIdentity::for_owner(
            function.into(),
            local,
            mir::LocalValueIdentityAuthority::Mir,
            identity,
        ));
    }

    fn observe_selector(
        &mut self,
        owner: hir::CallableMaterialization,
        selector: &hir::LocalValueSelector,
    ) {
        let path = match selector {
            hir::LocalValueSelector::LocalDeclaration { path }
            | hir::LocalValueSelector::BoundReceiver { path }
            | hir::LocalValueSelector::Synthetic { path, .. } => path,
            hir::LocalValueSelector::SuspensionResult { site } => site,
            hir::LocalValueSelector::This | hir::LocalValueSelector::Parameter { .. } => return,
        };
        let Some(segment) = path.segments().last() else {
            unreachable!("structural definition paths are non-empty")
        };
        let next = segment
            .ordinal()
            .checked_add(1)
            .expect("one callable cannot exhaust generated local ordinals");
        self.next_ordinals
            .entry((owner, segment.site_role()))
            .and_modify(|ordinal| *ordinal = (*ordinal).max(next))
            .or_insert(next);
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
            if entry.owner() != source.into() {
                remapped.push(entry);
                continue;
            }
            if let Some(wrapper_local) = wrapper_parameters.get(&entry.local()) {
                remapped.push(entry.relocated(source, *wrapper_local));
            }
            remapped.push(entry.relocated(driver, entry.local()));
        }
        self.entries = remapped;
    }

    pub(super) fn finish(self) -> mir::LocalValueIdentities {
        mir::LocalValueIdentities::checked(self.entries)
            .expect("MIR lowering records each persistent local location exactly once")
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
        let mut registry = LocalValueRegistry::default();
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
                .get(source, wrapper_parameter)
                .unwrap()
                .authority(),
            mir::LocalValueIdentityAuthority::Hir
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

    #[test]
    fn mir_generated_values_continue_existing_structural_ordinals() {
        let function = mir::FunctionId::from_raw(3_u32.into());
        let existing_local = mir::LocalId::from_raw(0_u32.into());
        let generated_local = mir::LocalId::from_raw(1_u32.into());
        let owner = record(0).key().owner();
        let existing = CborIdentityRecord::from_key(LocalValueKey::new(
            owner,
            LocalValueSelector::Synthetic {
                path: hir::StructuralDefinitionPath::from_first(
                    hir::StructuralPathSegment::new(
                        hir::StructuralDefinitionSiteRole::SyntheticValue,
                        0,
                    ),
                    [],
                ),
                role: hir::SyntheticLocalRole::Temporary,
            },
        ))
        .unwrap();
        let mut registry = LocalValueRegistry::default();
        registry.record(function, existing_local, &existing);
        registry.record_generated(
            function,
            owner,
            &[GeneratedLocalValue {
                local: generated_local,
                site_role: hir::StructuralDefinitionSiteRole::SyntheticValue,
                role: hir::SyntheticLocalRole::Temporary,
            }],
        );
        let identities = registry.finish();
        let generated = identities.get(function, generated_local).unwrap();

        assert_eq!(generated.authority(), mir::LocalValueIdentityAuthority::Mir);
        assert_ne!(generated.identity_record().id(), existing.id());
        assert!(matches!(
            generated.identity_record().key().selector(),
            LocalValueSelector::Synthetic { path, .. }
                if path.segments() == [hir::StructuralPathSegment::new(
                    hir::StructuralDefinitionSiteRole::SyntheticValue,
                    1,
                )]
        ));
    }
}
