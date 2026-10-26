use super::*;

#[cfg(test)]
mod tests;

/// Explicit raw storage retains all padding. Managed slots remain AS1 pointers,
/// including when zero, so SROA and statepoint rewriting preserve GC identity.
pub(super) fn storage_type<'ctx>(
    context: &'ctx Context,
    address_space: ManagedAddressSpace,
    size: u64,
    align: u64,
    scan: &RefScan,
) -> Result<StructType<'ctx>, CodegenError> {
    let mut fields = vec![alignment_anchor(context, align)?];
    let mut references = Vec::new();
    flatten_ref_scan(scan, &mut references);
    let mut cursor = 0;
    for offset in references {
        let end = offset
            .checked_add(8)
            .ok_or_else(|| CodegenError("MaybeUninit reference offset overflow".into()))?;
        if offset < cursor || offset % 8 != 0 || end > size {
            return Err(CodegenError(
                "MaybeUninit reference lies outside its storage".into(),
            ));
        }
        push_raw_storage(context, &mut fields, offset - cursor, align)?;
        fields.push(managed_ptr_ty(context, address_space).into());
        cursor = end;
    }
    push_raw_storage(context, &mut fields, size - cursor, align)?;
    Ok(context.struct_type(&fields, false))
}

fn push_raw_storage<'ctx>(
    context: &'ctx Context,
    fields: &mut Vec<BasicTypeEnum<'ctx>>,
    bytes: u64,
    align: u64,
) -> Result<(), CodegenError> {
    let width = align.min(8);
    let words = u32::try_from(bytes / width)
        .map_err(|_| CodegenError("MaybeUninit storage exceeds u32::MAX cells".into()))?;
    if words != 0 {
        // Integer cells preserve every bit without payload validity assumptions.
        // Byte arrays otherwise make SROA expose byte assembly to vectorization.
        let cell = match width {
            1 => context.i8_type(),
            2 => context.i16_type(),
            4 => context.i32_type(),
            _ => context.i64_type(),
        };
        fields.push(cell.array_type(words).into());
    }
    push_byte_padding(context, fields, bytes % width)
}
