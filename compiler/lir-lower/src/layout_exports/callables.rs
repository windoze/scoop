use super::*;

pub(super) fn lower(
    input: LayoutAbiExportInputV1<'_>,
    layouts: &lookup::Layouts<'_>,
) -> Result<lir::CanonicalExactCallableAbiExportsV1, Error> {
    let mut records = reserve(input.bridge.callables().entries().len())?;
    for binding in input.bridge.callables().entries() {
        let signature = binding.lowered_signature();
        let exact = signature.exact();
        let receiver = match exact.receiver().into_option() {
            None => lir::CallableAbiReceiverInputV1::NoReceiver,
            Some(exact) => lir::CallableAbiReceiverInputV1::Receiver(layouts.value(exact)?),
        };
        let mut parameters = reserve(exact.parameters().len())?;
        for exact in exact.parameters() {
            parameters.push(layouts.value(*exact)?);
        }
        let result = layouts.value(exact.result())?;
        records.push(
            crate::lower_exact_callable_abi_export(
                input.mir,
                input.lir,
                binding.implementation(),
                signature,
                lir::CallableAbiLayoutInputsV1 {
                    receiver,
                    parameters: &parameters,
                    result,
                },
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
