use super::*;

pub(super) fn lower(
    input: LayoutAbiExportInputV1<'_>,
) -> Result<lir::CanonicalExactCallableAbiExportsV1, Error> {
    let mut records = reserve(input.bridge.callables().entries().len())?;
    for binding in input.bridge.callables().entries() {
        let signature = binding.lowered_signature();
        records.push(
            crate::lower_exact_callable_abi_export(
                input.mir,
                input.lir,
                binding.implementation(),
                signature,
            )
            .map_err(|source| Error::Callable {
                target: binding.implementation(),
                source,
            })?,
        );
    }
    Ok(lir::CanonicalExactCallableAbiExportsV1::try_new(
        input.lir.module().meta.target_profile,
        input.lir.foundation(),
        records,
    )?)
}
