use super::*;
use inkwell::values::IntValue;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn integral_enum_tag(
        &self,
        value: IntValue<'ctx>,
        variants: &[u64],
        name: &str,
    ) -> Result<IntValue<'ctx>, CodegenError> {
        let tag_type = self.context.i64_type();
        let mut tag = tag_type.const_all_ones();
        for (index, bits) in variants.iter().enumerate() {
            let equal = self
                .builder
                .build_int_compare(
                    IntPredicate::EQ,
                    value,
                    value.get_type().const_int(*bits, false),
                    "enum_value",
                )
                .map_err(|error| CodegenError(format!("integral enum tag comparison: {error}")))?;
            tag = self
                .builder
                .build_select(equal, tag_type.const_int(index as u64, false), tag, name)
                .map_err(|error| CodegenError(format!("integral enum tag selection: {error}")))?
                .into_int_value();
        }
        Ok(tag)
    }
}
