//! Emit static enum fields in the same physical carrier used by enum_wrap.

use super::*;
use inkwell::values::PointerValue;

#[allow(clippy::too_many_arguments)]
pub(super) fn enum_constant<'ctx>(
    context: &'ctx Context,
    structs: &StructDefs,
    enums: &EnumDefs,
    globals: &BTreeMap<scoop_lir::GlobalId, GlobalValue<'ctx>>,
    managed_address_space: ManagedAddressSpace,
    target_data: &inkwell::targets::TargetData,
    expected: &LirType,
    variant: scoop_lir::LirVariantRef,
    fields: &[LirConstantImage],
) -> Result<BasicValueEnum<'ctx>, CodegenError> {
    if expected != &LirType::Enum(variant.definition()) || !enums.contains_variant(variant) {
        return Err(CodegenError(
            "enum constant does not match its storage type".into(),
        ));
    }
    let ty = basic_ty(context, structs, enums, managed_address_space, expected)?;
    let mut encoder = EnumConstantEncoder {
        context,
        structs,
        enums,
        globals,
        managed_address_space,
        target_data,
        bytes: Vec::new(),
        references: BTreeMap::new(),
    };
    match &enums[variant.definition()].repr {
        EnumRepr::Integral { values, .. } => {
            require_fields(fields, 0)?;
            Ok(ty
                .into_int_type()
                .const_int(values[variant.index() as usize], false)
                .into())
        }
        EnumRepr::Niche {
            kind,
            payload_variant,
        } => {
            if variant.index() == *payload_variant {
                require_fields(fields, 1)?;
                llvm_constant(
                    context,
                    structs,
                    enums,
                    globals,
                    managed_address_space,
                    target_data,
                    &kind.storage_type(),
                    &fields[0],
                )
            } else {
                require_fields(fields, 0)?;
                Ok(ty.const_zero())
            }
        }
        EnumRepr::Tagged { size, .. } => {
            let length = usize::try_from(*size).map_err(|_| {
                CodegenError("enum constant size exceeds addressable memory".into())
            })?;
            encoder.bytes.resize(length, 0);
            encoder.enumeration(variant, fields, 0)?;
            let mut offset = 0;
            let mut values = Vec::new();
            let ty = ty.into_struct_type();
            for field in ty.get_field_types() {
                let value = match field {
                    BasicTypeEnum::IntType(ty) => {
                        let bytes = encoder
                            .bytes
                            .get(offset..offset + 8)
                            .ok_or_else(bounds_error)?;
                        offset += 8;
                        ty.const_int(
                            u64::from_le_bytes(bytes.try_into().expect("the tag has eight bytes")),
                            false,
                        )
                        .into()
                    }
                    BasicTypeEnum::PointerType(ty) => {
                        let pointer = encoder
                            .references
                            .remove(&(offset as u64))
                            .unwrap_or_else(|| ty.const_null());
                        if pointer.get_type() != ty {
                            return Err(CodegenError(
                                "enum constant reference has the wrong pointer provenance".into(),
                            ));
                        }
                        offset += 8;
                        pointer.into()
                    }
                    BasicTypeEnum::ArrayType(ty) if ty.is_empty() => ty.const_zero().into(),
                    BasicTypeEnum::ArrayType(ty) => {
                        let end = offset
                            .checked_add(ty.len() as usize)
                            .ok_or_else(bounds_error)?;
                        let bytes = encoder.bytes.get(offset..end).ok_or_else(bounds_error)?;
                        offset = end;
                        context.const_string(bytes, false).into()
                    }
                    _ => return Err(CodegenError("invalid enum constant storage carrier".into())),
                };
                values.push(value);
            }
            if offset != length || !encoder.references.is_empty() {
                return Err(CodegenError(
                    "enum constant fields do not fit their storage carrier".into(),
                ));
            }
            Ok(ty.const_named_struct(&values).into())
        }
    }
}

struct EnumConstantEncoder<'a, 'ctx> {
    context: &'ctx Context,
    structs: &'a StructDefs,
    enums: &'a EnumDefs,
    globals: &'a BTreeMap<scoop_lir::GlobalId, GlobalValue<'ctx>>,
    managed_address_space: ManagedAddressSpace,
    target_data: &'a inkwell::targets::TargetData,
    bytes: Vec<u8>,
    references: BTreeMap<u64, PointerValue<'ctx>>,
}

impl<'ctx> EnumConstantEncoder<'_, 'ctx> {
    fn enumeration(
        &mut self,
        variant: scoop_lir::LirVariantRef,
        fields: &[LirConstantImage],
        base: u64,
    ) -> Result<(), CodegenError> {
        if !self.enums.contains_variant(variant) {
            return Err(CodegenError(
                "enum constant carries an invalid variant reference".into(),
            ));
        }
        match &self.enums[variant.definition()].repr {
            EnumRepr::Integral { kind, values } => {
                require_fields(fields, 0)?;
                self.write(
                    base,
                    &values[variant.index() as usize].to_le_bytes()
                        [..kind.width().bytes() as usize],
                )
            }
            EnumRepr::Niche {
                kind,
                payload_variant,
            } => {
                if variant.index() == *payload_variant {
                    require_fields(fields, 1)?;
                    self.field(&kind.storage_type(), &fields[0], base)
                } else {
                    require_fields(fields, 0)
                }
            }
            EnumRepr::Tagged { variants, .. } => {
                let representation = &variants[variant.index() as usize];
                require_fields(fields, representation.fields.len())?;
                self.write(base, &u64::from(variant.index()).to_le_bytes())?;
                for (field, layout) in fields.iter().zip(&representation.fields) {
                    self.field(
                        &layout.ty,
                        field,
                        base.checked_add(layout.offset).ok_or_else(bounds_error)?,
                    )?;
                }
                Ok(())
            }
        }
    }

    fn field(
        &mut self,
        ty: &LirType,
        value: &LirConstantImage,
        offset: u64,
    ) -> Result<(), CodegenError> {
        match value {
            LirConstantImage::Aggregate(fields) => {
                let LirType::Aggregate(types) = ty else {
                    return Err(CodegenError(
                        "tuple constant does not match its storage type".into(),
                    ));
                };
                require_fields(fields, types.len())?;
                let carrier = basic_ty(
                    self.context,
                    self.structs,
                    self.enums,
                    self.managed_address_space,
                    ty,
                )?
                .into_struct_type();
                for (index, (field, ty)) in fields.iter().zip(types).enumerate() {
                    let index = u32::try_from(index).map_err(|_| bounds_error())?;
                    let displacement = self
                        .target_data
                        .offset_of_element(&carrier, index)
                        .ok_or_else(bounds_error)?;
                    self.field(
                        ty,
                        field,
                        offset.checked_add(displacement).ok_or_else(bounds_error)?,
                    )?;
                }
                Ok(())
            }
            LirConstantImage::Enum { variant, fields }
                if ty == &LirType::Enum(variant.definition()) =>
            {
                self.enumeration(*variant, fields, offset)
            }
            LirConstantImage::Struct { struct_id, fields }
                if ty == &LirType::Struct(*struct_id) =>
            {
                let definition = &self.structs[*struct_id];
                require_fields(fields, definition.field_count())?;
                for (index, field) in fields.iter().enumerate() {
                    let ty = definition
                        .field_storage_type(index)
                        .expect("a checked struct field has its storage type");
                    let layout = definition
                        .field_layout(index)
                        .expect("a checked struct field has its layout");
                    self.field(
                        &ty,
                        field,
                        offset.checked_add(layout.offset).ok_or_else(bounds_error)?,
                    )?;
                }
                Ok(())
            }
            LirConstantImage::Integer(value) if ty == &value.scalar_type() => self.write(
                offset,
                &value.raw_bits().to_le_bytes()[..value.kind().width().bytes() as usize],
            ),
            LirConstantImage::Bool(value) if ty == &LirType::I1 => {
                self.write(offset, &[u8::from(*value)])
            }
            LirConstantImage::Float(scoop_lir::LirFloatConstant::F32(bits))
                if ty == &LirType::F32 =>
            {
                self.write(offset, &bits.to_le_bytes())
            }
            LirConstantImage::Float(scoop_lir::LirFloatConstant::F64(bits))
                if ty == &LirType::F64 =>
            {
                self.write(offset, &bits.to_le_bytes())
            }
            LirConstantImage::NullPointer(kind) if ty == &LirType::Ptr(*kind) => {
                self.write(offset, &[0; 8])
            }
            LirConstantImage::GlobalPointer {
                kind: PointerKind::Managed,
                ..
            } if ty == &LirType::Ptr(PointerKind::Managed) => {
                let pointer = llvm_constant(
                    self.context,
                    self.structs,
                    self.enums,
                    self.globals,
                    self.managed_address_space,
                    self.target_data,
                    ty,
                    value,
                )?
                .into_pointer_value();
                self.references.insert(offset, pointer);
                Ok(())
            }
            _ => Err(CodegenError(
                "enum constant field does not match its storage type".into(),
            )),
        }
    }

    fn write(&mut self, offset: u64, bytes: &[u8]) -> Result<(), CodegenError> {
        let start = usize::try_from(offset).map_err(|_| bounds_error())?;
        let end = start.checked_add(bytes.len()).ok_or_else(bounds_error)?;
        self.bytes
            .get_mut(start..end)
            .ok_or_else(bounds_error)?
            .copy_from_slice(bytes);
        Ok(())
    }
}

fn require_fields(fields: &[LirConstantImage], expected: usize) -> Result<(), CodegenError> {
    if fields.len() == expected {
        Ok(())
    } else {
        Err(CodegenError(
            "enum constant has the wrong field count".into(),
        ))
    }
}

fn bounds_error() -> CodegenError {
    CodegenError("enum constant field exceeds its storage extent".into())
}
