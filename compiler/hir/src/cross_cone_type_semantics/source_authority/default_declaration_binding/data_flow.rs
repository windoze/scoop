use super::*;
use scoop_identity::{
    PersistentFieldId, PersistentGenericTypeId, PersistentTypeId, SourceDeclarationKey,
};

pub(super) fn validate<'p, 's, 'a, 'f>(
    current: &BoundNominalParameterProtocolsV1<'p, 's, 'a, 'f>,
    dependencies: &[&BoundNominalParameterProtocolsV1<'p, 's, 'a, 'f>],
    template: &DefaultSourceTemplateV1,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), Error> {
    let input = DefaultBodyValidationInputV1::new(
        template.body(),
        template.locals(),
        template.receiver(),
        template.value_parameters(),
        template.result(),
        template.allows_suspend(),
        template.definition_path(),
    );
    input
        .validate_local_data_flow(
            &mut Authority {
                current,
                dependencies,
            },
            meter,
            path,
        )
        .map_err(|error| Error::DataFlow(Box::new(error)))
}

struct Authority<'d, 'p, 's, 'a, 'f> {
    current: &'d BoundNominalParameterProtocolsV1<'p, 's, 'a, 'f>,
    dependencies: &'d [&'d BoundNominalParameterProtocolsV1<'p, 's, 'a, 'f>],
}
impl DefaultBodyDataFlowAuthority<Error> for Authority<'_, '_, '_, '_, '_> {
    fn default_binding_struct_field_index(
        &mut self,
        declaration: PersistentFieldId,
        owner_type: &SignatureTypeKey,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<u32, Error> {
        meter.charge_work(64, path)?;
        let owner = match owner_type {
            SignatureTypeKey::Nominal(id) => SourceNominalId::Concrete(*id),
            SignatureTypeKey::NominalApplication { origin, .. } => {
                SourceNominalId::GenericTemplate(*origin)
            }
            _ => return Err(Error::BindingFieldOwner(declaration)),
        };
        let identities = self.current.members().nominals.foundation.identities;
        let key = match owner {
            SourceNominalId::Concrete(id) => {
                identities.canonical_key::<PersistentTypeId, SourceDeclarationKey>(id)
            }
            SourceNominalId::GenericTemplate(id) => {
                identities.canonical_key::<PersistentGenericTypeId, SourceDeclarationKey>(id)
            }
        }
        .map_err(|error| NominalSourceBindingError::Identity(error.to_string()))?;
        let provider =
            sources::provider(self.current, self.dependencies, key.origin(), meter, path)?;
        provider
            .members()
            .nominals
            .struct_field_index(owner, declaration, meter, path)
            .map_err(Into::into)
    }
}
