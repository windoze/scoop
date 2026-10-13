use scoop_lir as lir;
use scoop_mir as mir;

use crate::{LoweringContext, StorageResult, lir_type, safepoints};

mod leaves;
pub(crate) mod plans;
pub(crate) mod prepare;
pub(crate) use leaves::scalar_carrier;

pub(crate) use lir::AbiStorage as ValueStorage;

pub(crate) fn classify_storage(
    context: &LoweringContext,
    ty: lir::LirType,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
) -> StorageResult<ValueStorage> {
    let (size, alignment) = safepoints::lir_size_align(context, &ty, structs, enums)?;
    let scan = safepoints::root_scan(context, &ty, structs, enums, 0)?;
    if size == 0 {
        let layout = lir::AbiZeroSizedLayout::new(alignment)?;
        return Ok(ValueStorage::ZeroSized(lir::AbiZst::new(ty, layout)?));
    }
    let layout = lir::AbiNonZeroLayout::new(size, alignment)?;
    Ok(ValueStorage::NonZero(lir::AbiValue::new(ty, layout, scan)?))
}

#[cfg(test)]
mod tests_support;
#[cfg(test)]
pub(crate) use tests_support::{classify_argument, classify_signature};
