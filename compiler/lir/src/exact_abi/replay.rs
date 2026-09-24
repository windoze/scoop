use scoop_identity::{CallableBodyKey, PersistentExactTypeId, RepresentationRole};

use super::*;
use crate::ExternalStrongShapeSubjectV1;

pub(super) fn callable(
    target_profile: LirTargetProfile,
    target: StrongCallableDefinitionOwner,
    signature: ExactCallableSignature,
    protocol: ExactCallableProtocolV1,
    layouts: CallableAbiLayoutInputsV1<'_>,
    foundation: &OdrFreeLirFoundation,
    meter: &mut BudgetMeter,
) -> Result<ExactCallableAbiExportV1, ExactCallableAbiError> {
    let (signature, layouts) =
        self::signature(target_profile, signature, protocol, layouts, meter)?;
    let body = PersistentCallableBodyId::from_key(&CallableBodyKey::strong(target))?;
    meter.charge_work(foundation.callable_bodies().len() as u64, &WirePath::root())?;
    if !foundation
        .callable_bodies()
        .iter()
        .any(|record| record.id() == body)
    {
        return Err(ExactCallableAbiError::MissingCallableBody);
    }
    let physical = StrongShapeDefinitionRefV1::from_foundation(
        ExternalStrongShapeSubjectV1::Callable(target),
        foundation,
        meter,
    )?;
    let definition = StrongShapeDefinitionV1::from_callable_definition(target, physical)?
        .ok_or(ExactCallableAbiError::DefinitionSubject)?;
    Ok(ExactCallableAbiExportV1(Arc::new(CallableAbiBodyV1 {
        target,
        target_profile,
        signature,
        protocol,
        layouts,
        physical,
        definition,
    })))
}

pub(super) fn signature(
    target_profile: LirTargetProfile,
    signature: ExactCallableSignature,
    protocol: ExactCallableProtocolV1,
    layouts: CallableAbiLayoutInputsV1<'_>,
    meter: &mut BudgetMeter,
) -> Result<
    (
        CanonicalScoopAbiFunctionSignature,
        CallableAbiLayoutDependenciesV1,
    ),
    ExactCallableAbiError,
> {
    let path = WirePath::root();
    meter.charge_work(layouts.parameters.len() as u64 + 2, &path)?;
    if layouts.parameters.len() != signature.parameters().len() {
        return Err(ExactCallableAbiError::ParameterCount);
    }
    let receiver = match (layouts.receiver, signature.receiver().into_option()) {
        (CallableAbiReceiverInputV1::NoReceiver, None) => CallableAbiReceiverLayoutV1::NoReceiver,
        (CallableAbiReceiverInputV1::Receiver(layout), Some(exact)) => {
            CallableAbiReceiverLayoutV1::Receiver(value(layout, target_profile, exact)?)
        }
        _ => return Err(ExactCallableAbiError::Receiver),
    };
    let mut parameters = Vec::new();
    meter.try_reserve_collection_slots(&mut parameters, layouts.parameters.len(), &path)?;
    for (layout, exact) in layouts.parameters.iter().zip(signature.parameters()) {
        parameters.push(value(layout, target_profile, *exact)?);
    }
    let result = value(layouts.result, target_profile, signature.result())?;
    let mut arguments = Vec::new();
    let argument_count = parameters
        .len()
        .checked_add(usize::from(receiver.value().is_some()))
        .ok_or(ExactCallableAbiError::CountOverflow)?;
    meter.try_reserve_collection_slots(&mut arguments, argument_count, &path)?;
    for value in receiver
        .value()
        .into_iter()
        .chain(parameters.iter().map(AsRef::as_ref))
    {
        arguments.push(value.scoop_abi_argument(target_profile)?);
    }
    let result_passing = result.scoop_abi_return(target_profile)?;
    let signature = CanonicalScoopAbiFunctionSignature::new(
        signature,
        arguments,
        result_passing,
        protocol.gc_effect(),
    )?;
    Ok((
        signature,
        CallableAbiLayoutDependenciesV1 {
            receiver,
            parameters,
            result,
        },
    ))
}

fn value(
    layout: &ExactLayoutExportV1,
    target: LirTargetProfile,
    exact: PersistentExactTypeId,
) -> Result<Arc<ExactValueLayoutV1>, ExactCallableAbiError> {
    if layout.identity().exact() != exact {
        return Err(ExactCallableAbiError::ExactType);
    }
    if layout.identity().target() != target {
        return Err(ExactCallableAbiError::TargetProfile);
    }
    if layout.identity().layout_key().representation() != RepresentationRole::ManagedValue {
        return Err(ExactCallableAbiError::LayoutRole);
    }
    layout
        .value_handle()
        .ok_or(ExactCallableAbiError::LayoutRole)
}

#[derive(Debug)]
pub enum ExactCallableAbiError {
    MissingValueLayout { exact: PersistentExactTypeId },
    DuplicateValueLayout { exact: PersistentExactTypeId },
    ParameterCount,
    Receiver,
    CountOverflow,
    ExactType,
    TargetProfile,
    LayoutRole,
    MissingCallableBody,
    DefinitionSubject,
    Abi(scoop_identity::ScoopAbiError),
    Hash(scoop_wire::HashError),
    Definition(crate::StrongShapeDefinitionError),
    Resource(WireError),
}
macro_rules! from_error {
    ($source:ty, $variant:ident) => {
        impl From<$source> for ExactCallableAbiError {
            fn from(value: $source) -> Self {
                Self::$variant(value)
            }
        }
    };
}
from_error!(scoop_identity::ScoopAbiError, Abi);
from_error!(scoop_wire::HashError, Hash);
from_error!(crate::StrongShapeDefinitionError, Definition);
from_error!(WireError, Resource);
impl std::fmt::Display for ExactCallableAbiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "callable ABI replay failed: {self:?}")
    }
}
impl std::error::Error for ExactCallableAbiError {}
