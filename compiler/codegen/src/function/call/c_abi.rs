use super::*;
use inkwell::types::AnyType;
use scoop_lir::{CIndirectPassing, DirectCArgument, DirectCReturn, DirectCSignature};

impl<'ctx> FnEmitter<'_, 'ctx> {
    fn direct_c_signature(
        &self,
        destination: scoop_lir::CallDestination,
    ) -> Option<&DirectCSignature> {
        let scoop_lir::CallDestination::Extern(id) = destination else {
            return None;
        };
        match &self.extern_functions[id].kind {
            ExternFunctionKind::C {
                call_plan: scoop_lir::CAbiCallPlan::Direct(signature),
                ..
            } => Some(signature),
            _ => None,
        }
    }

    pub(super) fn c_indirect_argument_pointer(
        &self,
        destination: scoop_lir::CallDestination,
        index: usize,
        local: scoop_lir::LocalId,
    ) -> Result<PointerValue<'ctx>, CodegenError> {
        let pointer = self.local_pointer(local)?;
        if let Some(DirectCArgument::Indirect {
            value,
            passing: CIndirectPassing::ByValue { alignment },
        }) = self
            .direct_c_signature(destination)
            .and_then(|plan| plan.params.get(index))
            && alignment.get() > value.layout().alignment().get()
        {
            // Typed lowering already made a caller-owned copy. Align only
            // that local for the C ABI; packed type layout stays unchanged.
            pointer
                .as_instruction_value()
                .expect("local storage is an alloca")
                .set_alignment(alignment.get() as u32)
                .map_err(|error| CodegenError(format!("align C byval copy: {error}")))?;
        }
        Ok(pointer)
    }

    pub(in crate::function) fn apply_c_abi_attributes(
        &self,
        destination: scoop_lir::CallDestination,
        mut apply: impl FnMut(AttributeLoc, Attribute),
    ) -> Result<(), CodegenError> {
        let Some(signature) = self.direct_c_signature(destination) else {
            return Ok(());
        };
        let mut index = 0;
        match &signature.result {
            DirectCReturn::Indirect(value) => {
                self.c_storage_attributes(
                    0,
                    value,
                    Some("sret"),
                    value.layout().alignment().get(),
                    &mut apply,
                )?;
                index = 1;
            }
            DirectCReturn::Value(value) => {
                self.c_extension_attribute(AttributeLoc::Return, value.extension, &mut apply);
            }
            DirectCReturn::Void | DirectCReturn::DirectParts(_) => {}
        }
        for parameter in &signature.params {
            let count = match parameter {
                DirectCArgument::Scalar(value) => {
                    self.c_extension_attribute(
                        AttributeLoc::Param(index),
                        value.extension,
                        &mut apply,
                    );
                    1
                }
                DirectCArgument::DirectParts(parts) => parts.parts().len() as u32,
                DirectCArgument::Indirect { value, passing } => {
                    let (kind, alignment) = match passing {
                        CIndirectPassing::ByValue { alignment } => (Some("byval"), alignment.get()),
                        CIndirectPassing::CallerCopy => (None, value.layout().alignment().get()),
                    };
                    self.c_storage_attributes(index, value, kind, alignment, &mut apply)?;
                    1
                }
            };
            index = index
                .checked_add(count)
                .ok_or_else(|| CodegenError("C ABI parameter index exceeds u32::MAX".to_owned()))?;
        }
        // A source extern always calls its selected native symbol, including
        // names that LLVM would otherwise recognize as library builtins.
        apply(
            AttributeLoc::Function,
            self.context
                .create_enum_attribute(Attribute::get_named_enum_kind_id("nobuiltin"), 0),
        );
        Ok(())
    }

    fn c_storage_attributes(
        &self,
        index: u32,
        value: &scoop_lir::AbiValue,
        kind: Option<&str>,
        alignment: u64,
        apply: &mut impl FnMut(AttributeLoc, Attribute),
    ) -> Result<(), CodegenError> {
        if let Some(kind) = kind {
            let ty = basic_ty(
                self.context,
                self.structs,
                self.enums,
                self.managed_address_space,
                value.storage_type(),
            )?;
            apply(
                AttributeLoc::Param(index),
                self.context.create_type_attribute(
                    Attribute::get_named_enum_kind_id(kind),
                    ty.as_any_type_enum(),
                ),
            );
        }
        apply(
            AttributeLoc::Param(index),
            self.context
                .create_enum_attribute(Attribute::get_named_enum_kind_id("align"), alignment),
        );
        Ok(())
    }

    fn c_extension_attribute(
        &self,
        location: AttributeLoc,
        extension: scoop_lir::CIntegerExtension,
        apply: &mut impl FnMut(AttributeLoc, Attribute),
    ) {
        let name = match extension {
            scoop_lir::CIntegerExtension::None => return,
            scoop_lir::CIntegerExtension::Sign => "signext",
            scoop_lir::CIntegerExtension::Zero => "zeroext",
        };
        apply(
            location,
            self.context
                .create_enum_attribute(Attribute::get_named_enum_kind_id(name), 0),
        );
    }
}
