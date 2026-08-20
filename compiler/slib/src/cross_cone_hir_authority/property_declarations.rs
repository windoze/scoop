use super::*;
use scoop_identity::DefinitionOriginSubject;

impl CanonicalCrossConeHirSurfaceAuthority<'_> {
    pub(crate) fn validate_support_property_origins(
        &mut self,
    ) -> Result<(), CrossConeHirNominalAuthorityError> {
        for record in self
            .current_interface
            .property_interfaces()
            .support_records()
        {
            let subject = match record.declaration() {
                PropertyOwner::Property(id) => DefinitionOriginSubject::Property(id),
                PropertyOwner::ExtensionProperty(id) => {
                    DefinitionOriginSubject::ExtensionProperty(id)
                }
            };
            let key = self.property_key(record.declaration())?;
            self.validate_declaration_origin(
                &key,
                subject,
                record.owner(),
                record.declared_visibility(),
            )?;
        }
        Ok(())
    }
}
