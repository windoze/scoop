//! Mechanical callable ABI projection from a sealed MIR/LIR lowering pair.

use scoop_identity::{
    CallableBodyKey, CoreBuiltinNominal, ExactCallableSignature, ExactTypeKey,
    PersistentCallableBodyId, PersistentExactTypeId, StrongCallableDefinitionOwner,
};
use scoop_lir as lir;
use scoop_mir as mir;
use scoop_wire::{BudgetMeter, WireError, WirePath};

/// The containing layout/ABI section supplies the binding's lowered signature
/// and closes source roles and selected layout dependencies. This projection
/// proves their relation to the actual local function and emitted signature.
pub fn lower_exact_callable_abi_export(
    input: &mir::SingleConeStrongMirInput,
    output: &lir::SingleConeStrongLirOutput,
    target: StrongCallableDefinitionOwner,
    signature: &mir::MirBridgeCallableSignatureV1,
    layouts: lir::CallableAbiLayoutInputsV1<'_>,
    meter: &mut BudgetMeter,
) -> Result<lir::ExactCallableAbiExportV1, ExactCallableAbiLoweringError> {
    let path = WirePath::root();
    if input.module().cone != output.foundation().producer() {
        return Err(ExactCallableAbiLoweringError::Provider);
    }
    let bridges = input.production().strong_callable_bridges().bridges();
    meter.charge_work(bridges.len() as u64, &path)?;
    let bridge = bridges
        .iter()
        .find(|bridge| bridge.implementation() == target.callable_owner())
        .ok_or(ExactCallableAbiLoweringError::MissingMirSignature)?;
    if bridge.signature() != signature.exact() {
        return Err(ExactCallableAbiLoweringError::MirSignature);
    }
    let roots = input.materialization().callable_roots();
    meter.charge_work(roots.len() as u64, &path)?;
    let root = roots
        .iter()
        .find(|root| root.implementation() == target.callable_owner())
        .ok_or(ExactCallableAbiLoweringError::MissingMirBody)?;
    let function = &input.module().functions[root.function()];
    validate_source_signature(input.module(), function, signature.exact(), meter)?;
    if function.gc_effect != signature.gc_effect() {
        return Err(ExactCallableAbiLoweringError::GcEffect);
    }
    let body = PersistentCallableBodyId::from_key(&CallableBodyKey::strong(target))?;
    meter.charge_work(output.module().functions.len() as u64, &path)?;
    let physical = output
        .module()
        .functions
        .iter()
        .find(|function| function.callable_body.id() == body)
        .ok_or(ExactCallableAbiLoweringError::MissingLirBody)?;
    validate_physical_types(function, &physical.signature, meter)?;
    let protocol = match signature.gc_effect() {
        mir::GcEffect::Managed => lir::ExactCallableProtocolV1::OrdinaryManaged,
        mir::GcEffect::NoGc => lir::ExactCallableProtocolV1::OrdinaryNoGc,
    };
    meter.charge_collection_slots(signature.exact().parameters().len() as u64, &path)?;
    let record = lir::ExactCallableAbiExportV1::replay(
        output.module().meta.target_profile,
        target,
        signature.exact().clone(),
        protocol,
        layouts,
        output.foundation(),
        meter,
    )?;
    if record.definition().symbol() != physical.callable_body.symbol_request() {
        return Err(ExactCallableAbiLoweringError::Definition);
    }
    record.validate_physical_signature(
        &output.module().enums,
        &physical.signature,
        physical.gc_effect,
        meter,
    )?;
    Ok(record)
}

fn validate_physical_types(
    function: &mir::Function,
    signature: &lir::ScoopAbiSignature,
    meter: &mut BudgetMeter,
) -> Result<(), ExactCallableAbiLoweringError> {
    if function.params.len() != signature.arguments().len() {
        return Err(ExactCallableAbiLoweringError::PhysicalType);
    }
    for (parameter, argument) in function.params.iter().zip(signature.arguments()) {
        if !physical_type_matches(&parameter.ty, argument.logical_storage_type(), meter, 1)? {
            return Err(ExactCallableAbiLoweringError::PhysicalType);
        }
    }
    match signature.result().logical_storage_type() {
        None if function.return_ty == mir::Type::Unit => Ok(()),
        Some(actual) if physical_type_matches(&function.return_ty, actual, meter, 1)? => Ok(()),
        _ => Err(ExactCallableAbiLoweringError::PhysicalType),
    }
}

fn physical_type_matches(
    source: &mir::Type,
    actual: &lir::LirType,
    meter: &mut BudgetMeter,
    depth: u64,
) -> Result<bool, ExactCallableAbiLoweringError> {
    let path = WirePath::root();
    meter.check_semantic_depth(depth, &path)?;
    meter.charge_work(1, &path)?;
    if let mir::Type::Tuple(fields) = source {
        let lir::LirType::Aggregate(actual) = actual else {
            return Ok(false);
        };
        if fields.len() != actual.len() {
            return Ok(false);
        }
        for (source, actual) in fields.iter().zip(actual) {
            if !physical_type_matches(source, actual, meter, depth + 1)? {
                return Ok(false);
            }
        }
        Ok(true)
    } else {
        Ok(&crate::metadata::lir_type(source) == actual)
    }
}

fn validate_source_signature(
    module: &mir::Module,
    function: &mir::Function,
    signature: &ExactCallableSignature,
    meter: &mut BudgetMeter,
) -> Result<(), ExactCallableAbiLoweringError> {
    let path = WirePath::root();
    meter.charge_work(function.params.len() as u64, &path)?;
    let receiver = signature.receiver().into_option();
    if function.params.len()
        != signature
            .parameters()
            .len()
            .checked_add(usize::from(receiver.is_some()))
            .ok_or(ExactCallableAbiLoweringError::CountOverflow)?
    {
        return Err(ExactCallableAbiLoweringError::MirSignature);
    }
    for (parameter, exact) in function.params.iter().zip(
        receiver
            .into_iter()
            .chain(signature.parameters().iter().copied()),
    ) {
        if exact_type(module, &parameter.ty, meter)? != exact {
            return Err(ExactCallableAbiLoweringError::MirSignature);
        }
    }
    if exact_type(module, &function.return_ty, meter)? != signature.result() {
        return Err(ExactCallableAbiLoweringError::MirSignature);
    }
    Ok(())
}

fn exact_type(
    module: &mir::Module,
    ty: &mir::Type,
    meter: &mut BudgetMeter,
) -> Result<PersistentExactTypeId, ExactCallableAbiLoweringError> {
    let path = WirePath::root();
    meter.charge_work(module.meta.source_exact_types.len() as u64, &path)?;
    if let Some(exact) = module.meta.source_exact_types.get(ty) {
        return Ok(exact.identity_record().id());
    }
    let location = match ty {
        mir::Type::Any => {
            return Ok(PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
                CoreBuiltinNominal::Any.identity_record().id(),
            ))?);
        }
        mir::Type::Class(id) => mir::GeneratedExactTypeLocation::Class(*id),
        mir::Type::Enum(id, _) => mir::GeneratedExactTypeLocation::Enum(*id),
        _ => return Err(ExactCallableAbiLoweringError::MissingExactType),
    };
    meter.charge_work(module.meta.generated_exact_types.len() as u64, &path)?;
    module
        .meta
        .generated_exact_types
        .get(location)
        .map(|exact| exact.exact_record().id())
        .ok_or(ExactCallableAbiLoweringError::MissingExactType)
}

#[derive(Debug)]
pub enum ExactCallableAbiLoweringError {
    Provider,
    CountOverflow,
    MissingMirSignature,
    MirSignature,
    MissingMirBody,
    MissingLirBody,
    MissingExactType,
    PhysicalType,
    Definition,
    GcEffect,
    Abi(lir::ExactCallableAbiError),
    Physical(lir::ExactCallablePhysicalAbiError),
    Hash(scoop_wire::HashError),
    Resource(WireError),
}
macro_rules! from_error {
    ($source:ty, $variant:ident) => {
        impl From<$source> for ExactCallableAbiLoweringError {
            fn from(error: $source) -> Self {
                Self::$variant(error)
            }
        }
    };
}
from_error!(lir::ExactCallableAbiError, Abi);
from_error!(lir::ExactCallablePhysicalAbiError, Physical);
from_error!(scoop_wire::HashError, Hash);
from_error!(WireError, Resource);
impl std::fmt::Display for ExactCallableAbiLoweringError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "exact callable ABI projection failed: {self:?}")
    }
}
impl std::error::Error for ExactCallableAbiLoweringError {}
