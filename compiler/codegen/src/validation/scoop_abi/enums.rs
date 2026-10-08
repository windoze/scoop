//! Tagged and niche enum storage checks for classified Scoop ABI values.

use super::*;

impl AbiMetadataValidator<'_> {
    pub(super) fn enum_facts(
        &mut self,
        id: EnumDefId,
        owner: &str,
    ) -> Result<StorageFacts, CodegenError> {
        let index = arena_index(id);
        if index >= self.module.enums.len() {
            return Err(CodegenError(format!(
                "{owner} references invalid enum definition {index}"
            )));
        }
        if !self.visiting_enums.insert(id) {
            return Err(CodegenError(format!(
                "{owner} reaches a recursive by-value enum definition {index}"
            )));
        }
        let result = self.compute_enum_facts(id, owner);
        self.visiting_enums.remove(&id);
        result
    }

    fn compute_enum_facts(
        &mut self,
        id: EnumDefId,
        owner: &str,
    ) -> Result<StorageFacts, CodegenError> {
        struct VariantShape {
            fields: Vec<(LirType, u64)>,
            slot_offset: u64,
            slot_size: u64,
            slot_align: u64,
            gc_free: bool,
        }
        enum EnumShape {
            Niche(scoop_lir::NullNicheKind),
            Tagged {
                variants: Vec<VariantShape>,
                size: u64,
                align: u64,
            },
        }

        let (name, stored_scan, shape) = {
            let definition = &self.module.enums[id];
            let shape = match &definition.repr {
                EnumRepr::Niche { kind, .. } => EnumShape::Niche(*kind),
                EnumRepr::Tagged {
                    variants,
                    size,
                    align,
                } => EnumShape::Tagged {
                    variants: variants
                        .iter()
                        .map(|variant| VariantShape {
                            fields: variant
                                .fields
                                .iter()
                                .map(|field| (field.ty.clone(), field.offset))
                                .collect(),
                            slot_offset: variant.slot_offset,
                            slot_size: variant.slot_size,
                            slot_align: variant.slot_align,
                            gc_free: variant.gc_free,
                        })
                        .collect(),
                    size: *size,
                    align: *align,
                },
            };
            (definition.name.clone(), definition.scan.clone(), shape)
        };

        let expected = match shape {
            EnumShape::Niche(kind) => {
                let (size, align) = kind.layout(self.module.meta.target_profile);
                StorageFacts {
                    size,
                    align,
                    scan: if matches!(
                        kind,
                        scoop_lir::NullNicheKind::Managed | scoop_lir::NullNicheKind::Interface
                    ) {
                        RefScan::References(vec![0])
                    } else {
                        RefScan::None
                    },
                }
            }
            EnumShape::Tagged {
                variants,
                size: stored_size,
                align: stored_align,
            } => {
                struct ComputedVariant {
                    fields: Vec<StorageFacts>,
                    offsets: Vec<u64>,
                    size: u64,
                    align: u64,
                }
                let mut computed = Vec::with_capacity(variants.len());
                for (variant_index, variant) in variants.iter().enumerate() {
                    let mut fields = Vec::with_capacity(variant.fields.len());
                    for (field_index, (ty, _)) in variant.fields.iter().enumerate() {
                        fields.push(self.storage_facts(
                            ty,
                            &format!(
                                "{owner} enum `{name}` variant {variant_index} field {field_index}"
                            ),
                        )?);
                    }
                    let (offsets, size, align) = aggregate_layout(
                        fields.iter().map(|field| (field.size, field.align)),
                        owner,
                    )?;
                    if (variant.slot_size, variant.slot_align) != (size, align) {
                        return Err(CodegenError(format!(
                            "{owner} enum `{name}` variant {variant_index} slot layout {}/{} disagrees with target layout {size}/{align}",
                            variant.slot_size, variant.slot_align
                        )));
                    }
                    let expected_gc_free = fields.iter().all(|field| field.scan == RefScan::None);
                    if variant.gc_free != expected_gc_free {
                        return Err(CodegenError(format!(
                            "{owner} enum `{name}` variant {variant_index} gc_free metadata disagrees with its fields"
                        )));
                    }
                    computed.push(ComputedVariant {
                        fields,
                        offsets,
                        size,
                        align,
                    });
                }

                let pure_size = variants
                    .iter()
                    .zip(&computed)
                    .filter(|(variant, _)| variant.gc_free)
                    .map(|(_, variant)| variant.size)
                    .max()
                    .unwrap_or(0);
                let pure_align = variants
                    .iter()
                    .zip(&computed)
                    .filter(|(variant, _)| variant.gc_free)
                    .map(|(_, variant)| variant.align)
                    .max()
                    .unwrap_or(1);
                let tag = self
                    .module
                    .meta
                    .target_profile
                    .scalar_layout(scoop_lir::BackendScalarKind::I64);
                let pure_offset = checked_align_up(tag.size_bytes(), pure_align, owner)?;
                let mut cursor = pure_offset.checked_add(pure_size).ok_or_else(|| {
                    CodegenError(format!("{owner} enum `{name}` layout overflows u64"))
                })?;
                let mut align = tag.alignment_bytes().max(pure_align);
                let mut scans = Vec::new();
                for (variant_index, (variant, computed)) in
                    variants.iter().zip(&computed).enumerate()
                {
                    let slot_offset = if variant.gc_free {
                        pure_offset
                    } else {
                        cursor = checked_align_up(cursor, computed.align, owner)?;
                        let offset = cursor;
                        cursor = cursor.checked_add(computed.size).ok_or_else(|| {
                            CodegenError(format!("{owner} enum `{name}` layout overflows u64"))
                        })?;
                        offset
                    };
                    if variant.slot_offset != slot_offset {
                        return Err(CodegenError(format!(
                            "{owner} enum `{name}` variant {variant_index} slot offset {} disagrees with target offset {slot_offset}",
                            variant.slot_offset
                        )));
                    }
                    align = align.max(computed.align);
                    for (field_index, (((_, stored_offset), facts), relative_offset)) in variant
                        .fields
                        .iter()
                        .zip(&computed.fields)
                        .zip(&computed.offsets)
                        .enumerate()
                    {
                        let offset = if facts.size == 0 {
                            0
                        } else {
                            slot_offset.checked_add(*relative_offset).ok_or_else(|| {
                                CodegenError(format!(
                                    "{owner} enum `{name}` field offset overflows u64"
                                ))
                            })?
                        };
                        if *stored_offset != offset {
                            return Err(CodegenError(format!(
                                "{owner} enum `{name}` variant {variant_index} field {field_index} offset {stored_offset} disagrees with target offset {offset}"
                            )));
                        }
                        scans.push(shift_scan(&facts.scan, offset, owner)?);
                    }
                }
                let size = checked_align_up(cursor, align, owner)?;
                if (stored_size, stored_align) != (size, align) {
                    return Err(CodegenError(format!(
                        "{owner} enum `{name}` layout {stored_size}/{stored_align} disagrees with target layout {size}/{align}"
                    )));
                }
                StorageFacts {
                    size,
                    align,
                    scan: sequence_scans(scans),
                }
            }
        };
        if stored_scan != expected.scan {
            return Err(CodegenError(format!(
                "{owner} enum `{name}` scan {} disagrees with field-derived scan {}",
                stored_scan.dump(),
                expected.scan.dump()
            )));
        }
        Ok(expected)
    }
}
