use super::*;
use scoop_identity::{PersistentGenericTypeId, PersistentTypeId};

pub(super) fn query(count: usize, meter: &mut BudgetMeter, path: &WirePath) -> Result<(), Error> {
    meter.charge_work(u64::from(count.max(1).ilog2()) + 1, path)?;
    Ok(())
}
pub(super) fn provider<'d, 'p, 's, 'a, 'f>(
    current: &'d BoundNominalParameterProtocolsV1<'p, 's, 'a, 'f>,
    dependencies: &[&'d BoundNominalParameterProtocolsV1<'p, 's, 'a, 'f>],
    cone: ConeIdentity,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<&'d BoundNominalParameterProtocolsV1<'p, 's, 'a, 'f>, Error> {
    meter.charge_work(1, path)?;
    if current.provider() == cone {
        return Ok(current);
    }
    query(dependencies.len(), meter, path)?;
    dependencies
        .binary_search_by_key(&cone, |p| p.provider())
        .map(|index| dependencies[index])
        .map_err(|_| Error::MissingProvider(cone))
}
pub(super) fn payload<'d>(
    source: &'d BoundNominalParameterProtocolsV1<'_, '_, '_, '_>,
    declaration: CallableTemplateOrigin,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<&'d NominalSourceCallablePayloadV1, Error> {
    match declaration {
        CallableTemplateOrigin::Constructor(id) => {
            query(source.constructors().table().records().len(), meter, path)?;
            Ok(source.constructors().constructor_source(id)?.payload())
        }
        CallableTemplateOrigin::Function(_)
        | CallableTemplateOrigin::GenericFunction(_)
        | CallableTemplateOrigin::VariantConstructor(_) => {
            query(source.members().callables().records().len(), meter, path)?;
            Ok(source.members().callable_source(declaration)?.payload())
        }
        CallableTemplateOrigin::Accessor(_) => Err(Error::Declaration(declaration)),
    }
}
pub(super) fn binders(
    source: &BoundNominalParameterProtocolsV1<'_, '_, '_, '_>,
    payload: &NominalSourceCallablePayloadV1,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<DefaultTemplateProviderShapeV1, Error> {
    let nominals = source.members().nominals;
    query(nominals.table().records().len(), meter, path)?;
    let nominal = nominals.nominal_source(payload.owner())?;
    Ok(DefaultTemplateProviderShapeV1::try_new(
        nominal.type_parameters().len_u32(),
        payload.type_parameters().len_u32(),
    )?)
}
pub(super) struct Shapes<'d, 'a, 'f>(pub &'d BoundNominalSourceContractsV1<'a, 'f>);
impl NominalInterfaceShapeAuthority<NominalSourceBindingError> for Shapes<'_, '_, '_> {
    fn concrete_nominal_shape(
        &mut self,
        id: PersistentTypeId,
    ) -> Result<PublicNominalShapeV1, NominalSourceBindingError> {
        self.0
            .signature_nominal_shape(SourceNominalId::Concrete(id))
    }
    fn generic_nominal_shape(
        &mut self,
        id: PersistentGenericTypeId,
    ) -> Result<PublicNominalShapeV1, NominalSourceBindingError> {
        self.0
            .signature_nominal_shape(SourceNominalId::GenericTemplate(id))
    }
}

impl DefaultLocalFunctionSignatureAuthority<NominalSourceBindingError> for Shapes<'_, '_, '_> {
    fn default_local_function_own_binder_arity(
        &mut self,
        declaration: CallableTemplateOrigin,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<u32, NominalSourceBindingError> {
        use scoop_identity::{
            PersistentFunctionId, PersistentGenericFunctionId, SourceDeclarationKey,
        };
        let identities = self.0.foundation.identities;
        meter.charge_work(
            (u64::from(identities.identity_count().max(1).ilog2()) + 1) * 64,
            path,
        )?;
        let key = match declaration {
            CallableTemplateOrigin::Function(id) => {
                identities.canonical_key::<PersistentFunctionId, SourceDeclarationKey>(id)
            }
            CallableTemplateOrigin::GenericFunction(id) => {
                identities.canonical_key::<PersistentGenericFunctionId, SourceDeclarationKey>(id)
            }
            _ => {
                return Err(NominalSourceBindingError::Identity(format!(
                    "local signature target {declaration:?} is not a function"
                )));
            }
        }
        .map_err(|error| NominalSourceBindingError::Identity(error.to_string()))?;
        Ok(key.duplicate_signature().type_parameter_count())
    }
}
