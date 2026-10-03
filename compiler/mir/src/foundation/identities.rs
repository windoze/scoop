//! Reuse of current MIR records for later-stage identity queries.

use scoop_identity::{IdentityLayer, IdentityValidationError, PendingIdentityValidation};

use super::CanonicalMirFoundation;

impl CanonicalMirFoundation {
    pub fn register_identities(
        &self,
        identities: &mut PendingIdentityValidation,
    ) -> Result<(), IdentityValidationError> {
        macro_rules! register {
            ($($table:ident),+ $(,)?) => {
                $(for record in &self.$table {
                    identities.register_canonical(IdentityLayer::Mir, record.clone())?;
                })+
            };
        }
        register!(
            exact_types,
            generated_callables,
            generated_types,
            fields,
            enum_variants,
            enum_variant_fields,
            local_values,
            callback_applications,
            odr_groups,
            odr_members,
        );
        Ok(())
    }
}
