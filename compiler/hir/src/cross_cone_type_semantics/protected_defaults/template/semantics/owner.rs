use super::*;
use crate::{SignatureBinderScopeV1, SourceNominalId};
use scoop_identity::{CallableTemplateOrigin, NonEmptyVec, SignatureTypeKey};
use scoop_wire::WirePath;

pub(super) struct OwnerShape {
    pub scope: SignatureBinderScopeV1,
    pub binders: DefaultTemplateProviderShapeV1,
    pub receiver: Option<SignatureTypeKey>,
}
pub(super) fn shape<A: NominalInterfaceShapeAuthority<E>, E>(
    source: ProtectedDefaultOwnerSourceV1<'_>,
    authority: &mut A,
    meter: &mut BudgetMeter,
) -> Result<OwnerShape, ProtectedDefaultTemplateContractSemanticError<E>> {
    use ProtectedDefaultTemplateContractSemanticError as Error;
    let path = WirePath::root();
    meter.charge_work(1, &path).map_err(Error::Resource)?;
    let payload = source.payload();
    let arity = match payload.owner() {
        SourceNominalId::Concrete(id) => {
            let shape = authority
                .concrete_nominal_shape(id)
                .map_err(Error::Foundation)?;
            if shape.type_parameter_arity() != 0 {
                return Err(Error::OwnerShape);
            }
            0
        }
        SourceNominalId::GenericTemplate(id) => {
            let shape = authority
                .generic_nominal_shape(id)
                .map_err(Error::Foundation)?;
            if shape.type_parameter_arity() == 0 {
                return Err(Error::OwnerShape);
            }
            shape.type_parameter_arity()
        }
    };
    let own = payload.type_parameters().len_u32();
    let binders =
        DefaultTemplateProviderShapeV1::try_new(arity, own).map_err(Error::ProviderShape)?;
    let receiver = match source.declaration() {
        CallableTemplateOrigin::Constructor(_) | CallableTemplateOrigin::VariantConstructor(_) => {
            None
        }
        CallableTemplateOrigin::Function(_) | CallableTemplateOrigin::GenericFunction(_) => {
            meter
                .check_semantic_depth(1, &path)
                .map_err(Error::Resource)?;
            meter.charge_nodes(1, &path).map_err(Error::Resource)?;
            Some(match payload.owner() {
                SourceNominalId::Concrete(id) => SignatureTypeKey::Nominal(id),
                SourceNominalId::GenericTemplate(origin) => {
                    meter
                        .check_table_entries(u64::from(arity), &path)
                        .map_err(Error::Resource)?;
                    meter
                        .charge_work(u64::from(arity), &path)
                        .map_err(Error::Resource)?;
                    meter
                        .check_semantic_depth(2, &path)
                        .map_err(Error::Resource)?;
                    meter
                        .charge_nodes(u64::from(arity), &path)
                        .map_err(Error::Resource)?;
                    meter
                        .charge_edges(u64::from(arity), &path)
                        .map_err(Error::Resource)?;
                    let mut arguments = Vec::new();
                    meter
                        .try_reserve_collection_slots(&mut arguments, arity as usize, &path)
                        .map_err(Error::Resource)?;
                    arguments.extend((0..arity).map(|index| SignatureTypeKey::Binder {
                        depth: u32::from(own != 0),
                        index,
                    }));
                    SignatureTypeKey::NominalApplication {
                        origin,
                        arguments: NonEmptyVec::new(arguments).map_err(|_| Error::OwnerShape)?,
                    }
                }
            })
        }
        CallableTemplateOrigin::Accessor(_) => return Err(Error::OwnerShape),
    };
    Ok(OwnerShape {
        scope: binders.signature_scope(),
        binders,
        receiver,
    })
}
