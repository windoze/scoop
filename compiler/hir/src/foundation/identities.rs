//! Reuse of current HIR records for later-stage identity queries.

use scoop_identity::{IdentityLayer, IdentityValidationError, PendingIdentityValidation};

use super::CanonicalHirFoundation;

impl CanonicalHirFoundation {
    pub fn register_identities(
        &self,
        identities: &mut PendingIdentityValidation,
    ) -> Result<(), IdentityValidationError> {
        macro_rules! register {
            ($($table:ident),+ $(,)?) => {
                $(for record in &self.$table {
                    identities.register_canonical(IdentityLayer::Hir, record.clone())?;
                })+
            };
        }
        register!(
            types,
            generic_types,
            functions,
            generic_functions,
            constructors,
            properties,
            extension_properties,
            object_values,
            type_aliases,
            property_accessors,
            fields,
            enum_variants,
            enum_variant_fields,
            exact_types,
            export_bindings,
            callable_applications,
            generated_callables,
            generated_types,
            dispatch_slots,
            initialization_units,
            source_contexts,
            local_bindings,
            local_values,
            callback_registrations,
            odr_groups,
            odr_members,
        );
        for record in &self.source_native_contracts {
            identities.register_canonical_source_native_contract(IdentityLayer::Hir, record)?;
        }
        Ok(())
    }
}
