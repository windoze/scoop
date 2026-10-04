use super::*;

impl AbiMetadataValidator<'_> {
    pub(super) fn struct_facts(
        &mut self,
        id: StructDefId,
        owner: &str,
    ) -> Result<StorageFacts, CodegenError> {
        let index = arena_index(id);
        if index >= self.module.structs.len() {
            return Err(CodegenError(format!(
                "{owner} references invalid struct definition {index}"
            )));
        }
        if !self.visiting_structs.insert(id) {
            return Err(CodegenError(format!(
                "{owner} reaches a recursive by-value struct definition {index}"
            )));
        }
        let result = self.compute_struct_facts(id, owner);
        self.visiting_structs.remove(&id);
        result
    }

    fn compute_struct_facts(
        &mut self,
        id: StructDefId,
        owner: &str,
    ) -> Result<StorageFacts, CodegenError> {
        enum StructShape {
            Fields {
                fields: Vec<(LirType, scoop_lir::FieldLayout)>,
                packed: Option<u64>,
                explicit_align: Option<u64>,
                elide_zero_fields: bool,
            },
            Intrinsic(scoop_lir::IntrinsicTypeRepresentation),
        }

        let (name, stored_size, stored_align, shape) = {
            let definition = &self.module.structs[id];
            let shape = match &definition.representation {
                StructRepresentation::Scoop { fields } => StructShape::Fields {
                    fields: fields
                        .iter()
                        .map(|field| (field.ty.clone(), field.layout))
                        .collect(),
                    packed: None,
                    explicit_align: None,
                    elide_zero_fields: true,
                },
                StructRepresentation::C { contract, fields } => StructShape::Fields {
                    fields: fields
                        .iter()
                        .map(|field| (field.ty.storage_type(), field.layout))
                        .collect(),
                    packed: contract.packed.bytes(),
                    explicit_align: contract.aligned.bytes(),
                    elide_zero_fields: false,
                },
                StructRepresentation::Intrinsic(representation) => {
                    StructShape::Intrinsic(representation.clone())
                }
            };
            (
                definition.name.clone(),
                definition.size,
                definition.align,
                shape,
            )
        };

        let expected = match shape {
            StructShape::Fields {
                fields,
                packed,
                explicit_align,
                elide_zero_fields,
            } => {
                let mut field_facts = Vec::with_capacity(fields.len());
                for (index, (ty, _)) in fields.iter().enumerate() {
                    field_facts.push(
                        self.storage_facts(ty, &format!("{owner} struct `{name}` field {index}"))?,
                    );
                }
                let mut cursor = 0;
                let mut align = explicit_align.unwrap_or(1);
                let mut scans = Vec::new();
                for (index, ((_, stored), facts)) in fields.iter().zip(&field_facts).enumerate() {
                    let access_align = packed.map_or(facts.align, |cap| facts.align.min(cap));
                    let elided = elide_zero_fields && facts.size == 0;
                    let offset = if elided {
                        0
                    } else {
                        checked_align_up(cursor, access_align, owner)?
                    };
                    if stored.offset != offset || stored.access_align != access_align {
                        return Err(CodegenError(format!(
                            "{owner} struct `{name}` field {index} layout {}/{} disagrees with target layout {offset}/{access_align}",
                            stored.offset, stored.access_align
                        )));
                    }
                    if !elided {
                        scans.push(shift_scan(&facts.scan, offset, owner)?);
                        cursor = offset.checked_add(facts.size).ok_or_else(|| {
                            CodegenError(format!("{owner} struct `{name}` layout overflows u64"))
                        })?;
                    }
                    align = align.max(access_align);
                }
                StorageFacts {
                    size: checked_align_up(cursor, align, owner)?,
                    align,
                    scan: sequence_scans(scans),
                }
            }
            StructShape::Intrinsic(representation) => {
                let profile = self.module.meta.target_profile;
                let layout = match representation {
                    scoop_lir::IntrinsicTypeRepresentation::Integer(kind) => {
                        profile.integer_layout(kind)
                    }
                    scoop_lir::IntrinsicTypeRepresentation::Char => {
                        profile.scalar_layout(scoop_lir::BackendScalarKind::I32)
                    }
                    scoop_lir::IntrinsicTypeRepresentation::Boolean => {
                        profile.scalar_layout(scoop_lir::BackendScalarKind::I1)
                    }
                    scoop_lir::IntrinsicTypeRepresentation::Ptr { .. } => {
                        profile.pointer_layout(PointerKind::Raw)
                    }
                    scoop_lir::IntrinsicTypeRepresentation::FunPtr { .. } => {
                        profile.pointer_layout(PointerKind::Code)
                    }
                    scoop_lir::IntrinsicTypeRepresentation::String => {
                        return Err(CodegenError(format!(
                            "{owner} uses intrinsic String declaration `{name}` as value storage"
                        )));
                    }
                };
                StorageFacts {
                    size: layout.size_bytes(),
                    align: layout.alignment_bytes(),
                    scan: RefScan::None,
                }
            }
        };
        if (stored_size, stored_align) != (expected.size, expected.align) {
            return Err(CodegenError(format!(
                "{owner} struct `{name}` layout {stored_size}/{stored_align} disagrees with target layout {}/{}",
                expected.size, expected.align
            )));
        }
        Ok(expected)
    }
}
