use super::*;
use crate::{SignatureBinderScopeV1, SourceNominalId};
use scoop_identity::SignatureTypeKey;
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
    let root = crate::PersistentLexicalRootV1::try_from(source.declaration())
        .map_err(|_| Error::OwnerShape)?;
    let receiver = binders
        .nominal_source_receiver(root, payload.owner(), meter, &path)
        .map_err(|error| match error {
            crate::DefaultNominalReceiverBuildError::Resource(error) => Error::Resource(error),
            crate::DefaultNominalReceiverBuildError::OwnerShape => Error::OwnerShape,
        })?;
    Ok(OwnerShape {
        scope: binders.signature_scope(),
        binders,
        receiver,
    })
}
