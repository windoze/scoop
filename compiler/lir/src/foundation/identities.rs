//! Reuse of current LIR records for layout and ABI queries.

use scoop_identity::{IdentityLayer, IdentityValidationError, PendingIdentityValidation};

use super::CanonicalLirFoundation;

impl CanonicalLirFoundation {
    pub fn register_identities(
        &self,
        identities: &mut PendingIdentityValidation,
    ) -> Result<(), IdentityValidationError> {
        macro_rules! register {
            ($($table:ident),+ $(,)?) => {
                $(for record in &self.$table {
                    identities.register_canonical(IdentityLayer::Lir, record.clone())?;
                })+
            };
        }
        register!(
            layouts,
            scans,
            dispatch_tables,
            static_storages,
            immortal_objects,
            odr_groups,
            odr_members,
            safepoint_sites,
            bridge_units,
            bridge_atoms,
            native_link_requirements,
            definition_plans,
            definition_atoms,
        );
        for record in &self.callable_bodies {
            identities.register_canonical_callable_body(IdentityLayer::Lir, record)?;
        }
        for record in &self.c_abi_signatures {
            identities.register_canonical_c_abi_signature(IdentityLayer::Lir, record)?;
        }
        for record in &self.c_abi_layouts {
            identities.register_canonical_c_abi_layout(IdentityLayer::Lir, record)?;
        }
        let mut contracts = std::collections::BTreeSet::new();
        for record in &self.native_contracts {
            if contracts.insert(record.fingerprint()) {
                identities.register_canonical_native_contract(IdentityLayer::Lir, record)?;
            }
        }
        Ok(())
    }
}
