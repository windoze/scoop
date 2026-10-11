use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn physical_call_arguments(
        &self,
        destination: scoop_lir::CallDestination,
        arguments: &[scoop_lir::AbiCallArgument],
        signature: &scoop_lir::ScoopAbiSignature,
        result: &TypedCallResult<'_>,
        live: Option<&MaterializedStatepointLive<'ctx>>,
    ) -> Result<Vec<BasicValueEnum<'ctx>>, CodegenError> {
        let mut values = Vec::with_capacity(
            arguments.len() + usize::from(matches!(result, TypedCallResult::Indirect { .. })),
        );
        if let TypedCallResult::Indirect { storage, .. } = result {
            values.push(self.local_pointer(*storage)?.into());
        }
        for (index, (argument, convention)) in
            arguments.iter().zip(signature.arguments()).enumerate()
        {
            match *argument {
                scoop_lir::AbiCallArgument::ElidedZst(_) => {}
                scoop_lir::AbiCallArgument::Direct(logical) => {
                    let value = self.typed_call_argument_value(logical, live)?;
                    if let scoop_lir::AbiArgument::Direct(scoop_lir::AbiDirectValue::DirectParts(
                        parts,
                    )) = convention
                    {
                        let mut physical = self.outgoing_parts(parts, value)?;
                        if parts.coercion() == scoop_lir::AbiCoercion::interface()
                            && let Some(object) =
                                live.and_then(|live| live.interface_object(logical))
                        {
                            physical[0] = object.into();
                        }
                        values.extend(physical);
                    } else {
                        values.push(value);
                    }
                }
                scoop_lir::AbiCallArgument::Indirect(storage) => {
                    values.push(
                        self.c_indirect_argument_pointer(destination, index, storage.local())?
                            .into(),
                    );
                }
            }
        }
        Ok(values)
    }
}
