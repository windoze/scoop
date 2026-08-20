use super::*;
use inkwell::intrinsics::Intrinsic;
use scoop_lir::LirFloatConversion as FloatConversion;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn float_conversion(
        &self,
        conversion: LirFloatConversion,
        operand: BasicValueEnum<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        match conversion {
            FloatConversion::FromInteger { source, target } => {
                let operand = operand.into_int_value();
                let target = float_type(self.context, target);
                let result = match source.signedness() {
                    IntegerSignedness::Signed => {
                        self.builder
                            .build_signed_int_to_float(operand, target, "int.to_float")
                    }
                    IntegerSignedness::Unsigned => {
                        self.builder
                            .build_unsigned_int_to_float(operand, target, "uint.to_float")
                    }
                };
                Ok(result.map_err(float_error)?.into())
            }
            FloatConversion::ToInteger { source, target } => {
                let name = match target.signedness() {
                    IntegerSignedness::Signed => "llvm.fptosi.sat",
                    IntegerSignedness::Unsigned => "llvm.fptoui.sat",
                };
                let target = integer_ty(self.context, target.width());
                let source = float_type(self.context, source);
                let declaration = Intrinsic::find(name)
                    .and_then(|intrinsic| {
                        intrinsic.get_declaration(self.llvm, &[target.into(), source.into()])
                    })
                    .ok_or_else(|| CodegenError(format!("LLVM lacks required intrinsic {name}")))?;
                // Saturate at the requested width; narrowing an i64 result is incorrect.
                self.builder
                    .build_call(declaration, &[operand.into()], "float.to_int")
                    .map_err(float_error)?
                    .try_as_basic_value()
                    .basic()
                    .ok_or_else(|| CodegenError(format!("{name} must return its target integer")))
            }
            FloatConversion::BetweenFloats { source, target } => {
                if source == target {
                    return Ok(operand);
                }
                let operand = operand.into_float_value();
                let target_ty = float_type(self.context, target);
                let result = if source.bits() < target.bits() {
                    self.builder
                        .build_float_ext(operand, target_ty, "float.extend")
                } else {
                    self.builder
                        .build_float_trunc(operand, target_ty, "float.truncate")
                };
                Ok(result.map_err(float_error)?.into())
            }
        }
    }
}
