use super::*;
use scoop_lir::{BoxPayload, BoxValuePlace, BoxedValueDescriptor, UnboxResult};

impl AbiMetadataValidator<'_> {
    pub(super) fn validate_boxing(&mut self, function: &Function) -> Result<(), CodegenError> {
        for (_, block) in function.blocks.iter() {
            for instruction in &block.instructions {
                match instruction {
                    Instruction::BoxValue { out, payload, .. } => {
                        self.box_temp(function, *out, &scoop_lir::MANAGED_PTR)?;
                        match payload {
                            BoxPayload::ZeroSized(descriptor) => {
                                self.box_zst(descriptor, function.symbol())?;
                            }
                            BoxPayload::NonZero(place) => self.box_place(place, function)?,
                        }
                    }
                    Instruction::UnboxValue { object, result } => {
                        if function.value_ty(&self.module.globals, *object)
                            != scoop_lir::MANAGED_PTR
                        {
                            return Err(call_error(function, "unbox requires a managed object"));
                        }
                        match result {
                            UnboxResult::ZeroSized { descriptor, out } => {
                                self.box_zst(descriptor, function.symbol())?;
                                self.box_temp(
                                    function,
                                    *out,
                                    descriptor.value().representation().storage_type(),
                                )?;
                            }
                            UnboxResult::NonZero(place) => self.box_place(place, function)?,
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn box_zst(
        &mut self,
        descriptor: &scoop_lir::BoxedZstDescriptor,
        owner: &str,
    ) -> Result<(), CodegenError> {
        let value = descriptor.value();
        self.validate_zst(value.representation(), owner)?;
        let rebuilt = BoxedValueDescriptor::from_local(
            &self.module.meta.type_descriptors,
            descriptor.descriptor(),
            value.exact(),
            value.representation().storage_type().clone(),
        );
        if rebuilt != Ok(BoxedValueDescriptor::ZeroSized(descriptor.clone())) {
            return Err(CodegenError(format!(
                "box/unbox @{owner} has an inconsistent BoxedValue descriptor"
            )));
        }
        Ok(())
    }

    fn box_place(
        &mut self,
        place: &BoxValuePlace,
        function: &Function,
    ) -> Result<(), CodegenError> {
        let descriptor = place.descriptor();
        self.validate_value(descriptor.value(), function.symbol())?;
        let rebuilt = BoxedValueDescriptor::from_local(
            &self.module.meta.type_descriptors,
            descriptor.descriptor(),
            descriptor.payload_exact(),
            descriptor.value().storage_type().clone(),
        );
        if rebuilt != Ok(BoxedValueDescriptor::NonZero(descriptor.clone()))
            || descriptor
                .clone()
                .bind_place(&function.locals, place.local())
                .as_ref()
                != Ok(place)
        {
            return Err(call_error(
                function,
                "box/unbox storage or BoxedValue descriptor is inconsistent",
            ));
        }
        Ok(())
    }

    fn box_temp(
        &self,
        function: &Function,
        out: TempId,
        expected: &LirType,
    ) -> Result<(), CodegenError> {
        if arena_index(out) >= function.temps.len() || &function.temps[out].ty != expected {
            return Err(call_error(
                function,
                "box/unbox result temporary has the wrong type",
            ));
        }
        Ok(())
    }
}
