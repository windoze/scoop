use super::*;

/// A release body belongs to the nominal, independently of its constructors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportReleaseTemplateV1 {
    definition_origin: ExportDefinitionSourceV1,
    body: ExportTemplateFragmentV1,
}

impl ExportReleaseTemplateV1 {
    pub fn try_new(
        definition_origin: ExportDefinitionSourceV1,
        body: ExportTemplateFragmentV1,
    ) -> Result<Self, GenericInitializationBuildError> {
        require_body(&body)?;
        if body.locals().records().iter().any(|local| {
            matches!(
                local.selector(),
                LocalValueSelector::This | LocalValueSelector::Parameter { .. }
            )
        }) {
            return Err(GenericInitializationBuildError::ReleaseInputs);
        }
        Ok(Self {
            definition_origin,
            body,
        })
    }

    pub const fn definition_origin(&self) -> &ExportDefinitionSourceV1 {
        &self.definition_origin
    }

    pub const fn body(&self) -> &ExportTemplateFragmentV1 {
        &self.body
    }
}

impl CanonicalExportGenericInitializationsV1 {
    pub(crate) fn validate_release_policies(
        &self,
        nominals: &crate::CanonicalNominalInterfacesV1,
    ) -> Result<(), PersistentGenericTypeId> {
        let declared = |record: &crate::NominalInterfaceRecordV1| {
            matches!(
                record.declaration_details().release_policy(),
                crate::NominalReleasePolicyV1::SynchronousGcFree { .. }
            )
        };
        for initialization in self.records() {
            if initialization.release_policy().hook().is_some()
                && !nominals
                    .declaration(crate::SourceNominalId::GenericTemplate(
                        initialization.owner(),
                    ))
                    .is_some_and(declared)
            {
                return Err(initialization.owner());
            }
        }
        for record in nominals.all_records().filter(|record| declared(record)) {
            if let crate::SourceNominalId::GenericTemplate(owner) = record.declaration()
                && !self
                    .get(owner)
                    .is_some_and(|record| record.release_policy().hook().is_some())
            {
                return Err(owner);
            }
        }
        Ok(())
    }
}
