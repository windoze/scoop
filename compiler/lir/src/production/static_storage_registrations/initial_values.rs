//! Encode static initial bytes and immortal relocations using resolved layouts.

use super::*;

pub(super) struct StaticInitialStateEncoder<'a> {
    pub(super) globals: &'a la_arena::Arena<crate::Global>,
    pub(super) structs: &'a crate::StructDefs,
    pub(super) enums: &'a crate::EnumDefs,
    pub(super) storage: PersistentStaticStorageId,
    pub(super) template: &'a mut [u8],
    pub(super) relocations: &'a mut Vec<StaticImmortalRelocationPlanV1>,
}

impl StaticInitialStateEncoder<'_> {
    pub(super) fn collect(
        &mut self,
        expected: &LirType,
        value: &LirConstantImage,
        base_offset: u64,
    ) -> Result<(), StrongStaticStorageSemanticPlanBuildError> {
        let storage = self.storage;
        let type_mismatch = || StrongStaticStorageSemanticPlanBuildError::InitialValue {
            storage,
            kind: StaticStorageInitialValueFailureV1::TypeMismatch,
        };
        match value {
            LirConstantImage::Integer(value) if &value.scalar_type() == expected => {
                let width = usize::try_from(value.kind().width().bits() / 8)
                    .expect("source integer widths fit usize");
                self.write(base_offset, &value.raw_bits().to_le_bytes()[..width])
            }
            LirConstantImage::Float(crate::LirFloatConstant::F32(bits))
                if expected == &LirType::F32 =>
            {
                self.write(base_offset, &bits.to_le_bytes())
            }
            LirConstantImage::Float(crate::LirFloatConstant::F64(bits))
                if expected == &LirType::F64 =>
            {
                self.write(base_offset, &bits.to_le_bytes())
            }
            LirConstantImage::Bool(value) if expected == &LirType::I1 => {
                self.write(base_offset, &[u8::from(*value)])
            }
            LirConstantImage::NullPointer(kind) if expected == &LirType::Ptr(*kind) => Ok(()),
            LirConstantImage::GlobalPointer { global, kind }
                if expected == &LirType::Ptr(*kind) && *kind == PointerKind::Managed =>
            {
                let index = global.into_raw().into_u32() as usize;
                if index >= self.globals.len() {
                    return Err(StrongStaticStorageSemanticPlanBuildError::InitialValue {
                        storage,
                        kind: StaticStorageInitialValueFailureV1::MissingGlobal,
                    });
                }
                let target = &self.globals[*global];
                let GlobalInit::StringConst { identity, .. } = &target.init else {
                    return Err(StrongStaticStorageSemanticPlanBuildError::InitialValue {
                        storage,
                        kind: StaticStorageInitialValueFailureV1::NonImmortalTarget,
                    });
                };
                if target.address_kind != PointerKind::Managed {
                    return Err(StrongStaticStorageSemanticPlanBuildError::InitialValue {
                        storage,
                        kind: StaticStorageInitialValueFailureV1::TargetAddressKind,
                    });
                }
                self.relocations.push(StaticImmortalRelocationPlanV1 {
                    pointer_offset: base_offset,
                    target: identity.identity_record().id(),
                });
                Ok(())
            }
            LirConstantImage::Enum { variant, fields } => {
                let LirType::Enum(expected_enum) = expected else {
                    return Err(type_mismatch());
                };
                if variant.definition() != *expected_enum || !self.enums.contains_variant(*variant)
                {
                    return Err(StrongStaticStorageSemanticPlanBuildError::InitialValue {
                        storage,
                        kind: StaticStorageInitialValueFailureV1::InvalidEnumVariant,
                    });
                }
                let field_count = match &self.enums[*expected_enum].repr {
                    EnumRepr::Integral { .. } => 0,
                    EnumRepr::Niche {
                        payload_variant, ..
                    } => usize::from(variant.index() == *payload_variant),
                    EnumRepr::Tagged { variants, .. } => {
                        variants[variant.index() as usize].fields.len()
                    }
                };
                if fields.len() != field_count {
                    return Err(StrongStaticStorageSemanticPlanBuildError::InitialValue {
                        storage,
                        kind: StaticStorageInitialValueFailureV1::EnumFieldCount,
                    });
                }
                match &self.enums[*expected_enum].repr {
                    EnumRepr::Niche { kind, .. } => {
                        if let Some(field) = fields.first() {
                            self.collect(&kind.storage_type(), field, base_offset)?;
                        }
                        Ok(())
                    }
                    EnumRepr::Integral { kind, values } => self.write(
                        base_offset,
                        &values[variant.index() as usize].to_le_bytes()
                            [..kind.width().bytes() as usize],
                    ),
                    EnumRepr::Tagged { variants, .. } => {
                        self.write(base_offset, &u64::from(variant.index()).to_le_bytes())?;
                        for (field, layout) in fields
                            .iter()
                            .zip(&variants[variant.index() as usize].fields)
                        {
                            let offset = base_offset.checked_add(layout.offset).ok_or(
                                StrongStaticStorageSemanticPlanBuildError::InitialValue {
                                    storage,
                                    kind: StaticStorageInitialValueFailureV1::OffsetOverflow,
                                },
                            )?;
                            self.collect(&layout.ty, field, offset)?;
                        }
                        Ok(())
                    }
                }
            }
            LirConstantImage::Struct { struct_id, fields } => {
                if expected != &LirType::Struct(*struct_id) {
                    return Err(type_mismatch());
                }
                let index = struct_id.into_raw().into_u32() as usize;
                if index >= self.structs.len() {
                    return Err(StrongStaticStorageSemanticPlanBuildError::InitialValue {
                        storage,
                        kind: StaticStorageInitialValueFailureV1::UnknownStruct,
                    });
                }
                let definition = &self.structs[*struct_id];
                if fields.len() != definition.field_count() {
                    return Err(StrongStaticStorageSemanticPlanBuildError::InitialValue {
                        storage,
                        kind: StaticStorageInitialValueFailureV1::StructFieldCount,
                    });
                }
                for (index, field) in fields.iter().enumerate() {
                    let field_type = definition.field_storage_type(index).ok_or(
                        StrongStaticStorageSemanticPlanBuildError::InitialValue {
                            storage,
                            kind: StaticStorageInitialValueFailureV1::UnknownStruct,
                        },
                    )?;
                    let offset = definition
                        .field_layout(index)
                        .and_then(|layout| base_offset.checked_add(layout.offset))
                        .ok_or(StrongStaticStorageSemanticPlanBuildError::InitialValue {
                            storage,
                            kind: StaticStorageInitialValueFailureV1::OffsetOverflow,
                        })?;
                    self.collect(&field_type, field, offset)?;
                }
                Ok(())
            }
            _ => Err(type_mismatch()),
        }
    }

    fn write(
        &mut self,
        offset: u64,
        bytes: &[u8],
    ) -> Result<(), StrongStaticStorageSemanticPlanBuildError> {
        let storage = self.storage;
        let start = usize::try_from(offset).map_err(|_| {
            StrongStaticStorageSemanticPlanBuildError::InitialValue {
                storage,
                kind: StaticStorageInitialValueFailureV1::TemplateBounds,
            }
        })?;
        let end = start.checked_add(bytes.len()).ok_or(
            StrongStaticStorageSemanticPlanBuildError::InitialValue {
                storage,
                kind: StaticStorageInitialValueFailureV1::TemplateBounds,
            },
        )?;
        let target = self.template.get_mut(start..end).ok_or(
            StrongStaticStorageSemanticPlanBuildError::InitialValue {
                storage,
                kind: StaticStorageInitialValueFailureV1::TemplateBounds,
            },
        )?;
        target.copy_from_slice(bytes);
        Ok(())
    }
}
