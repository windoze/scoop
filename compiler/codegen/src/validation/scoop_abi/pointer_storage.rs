use super::*;

impl AbiMetadataValidator<'_> {
    pub(super) fn validate_pointer_storage(
        &mut self,
        function: &Function,
    ) -> Result<(), CodegenError> {
        for (_, block) in function.blocks.iter() {
            for instruction in &block.instructions {
                match instruction {
                    Instruction::MakeZstValue { out, value } => {
                        self.validate_zst(value.representation(), function.symbol())?;
                        validate_result(function, *out, value.representation().storage_type())?;
                    }
                    Instruction::RawLoad { out, pointee, .. } => {
                        self.validate_value(pointee, function.symbol())?;
                        validate_result(function, *out, pointee.storage_type())?;
                    }
                    Instruction::RawStore { value, pointee, .. } => {
                        self.validate_value(pointee, function.symbol())?;
                        if function.value_ty(&self.module.globals, *value)
                            != *pointee.storage_type()
                        {
                            return Err(call_error(
                                function,
                                "raw store value disagrees with its pointee storage",
                            ));
                        }
                    }
                    _ => continue,
                }
            }
        }
        Ok(())
    }
}

fn validate_result(
    function: &Function,
    out: TempId,
    expected: &LirType,
) -> Result<(), CodegenError> {
    if arena_index(out) >= function.temps.len() || &function.temps[out].ty != expected {
        return Err(call_error(
            function,
            "pointer storage result temporary has the wrong type",
        ));
    }
    Ok(())
}
