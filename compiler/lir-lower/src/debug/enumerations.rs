use super::*;

impl Types<'_> {
    pub(super) fn enumeration(&mut self, id: mir::EnumId) -> StorageResult<DebugTypeKind> {
        let definition = &self.module.enums[id];
        Ok(match &self.enums[enum_def_id(id)].repr {
            lir::EnumRepr::Integral { kind, values } => DebugTypeKind::Enumeration {
                signed: kind.signedness() == lir::IntegerSignedness::Signed,
                variants: definition
                    .variants
                    .iter()
                    .zip(values)
                    .map(|(variant, value)| (variant.name.clone(), *value))
                    .collect(),
            },
            lir::EnumRepr::Tagged { variants, .. } => {
                let word = self.context.machine_scalar_layout();
                let tag = inline(DebugType {
                    name: format!("{}::Tag", definition.name),
                    size: word.size,
                    align: word.align,
                    kind: DebugTypeKind::Enumeration {
                        signed: false,
                        variants: definition
                            .variants
                            .iter()
                            .enumerate()
                            .map(|(index, variant)| (variant.name.clone(), index as u64))
                            .collect(),
                    },
                });
                let mut fields = vec![field("__scoop_tag", 0, tag)];
                for (variant, layout) in definition.variants.iter().zip(variants) {
                    let mut members = Vec::new();
                    for (member, physical) in variant.fields.iter().zip(&layout.fields) {
                        // Zero-sized fields have no payload and their canonical
                        // enum-relative offset is zero.
                        let offset = physical.offset.saturating_sub(layout.slot_offset);
                        members.push(field(&member.name, offset, self.source(&member.ty)?));
                    }
                    fields.push(field(
                        &variant.name,
                        layout.slot_offset,
                        inline(DebugType {
                            name: format!("{}::{}", definition.name, variant.name),
                            size: layout.slot_size,
                            align: layout.slot_align,
                            kind: DebugTypeKind::Record(members),
                        }),
                    ));
                }
                DebugTypeKind::Record(fields)
            }
            lir::EnumRepr::Niche {
                payload_variant,
                kind,
            } => {
                let variant = &definition.variants[*payload_variant as usize];
                let payload = &variant.fields[0];
                let source = self.source(&payload.ty)?;
                let mut fields = vec![field("__scoop_niche", 0, source.clone())];
                for (index, variant) in definition.variants.iter().enumerate() {
                    let (size, align, fields_) = if index == *payload_variant as usize {
                        let (size, align) = kind.layout(self.context.target_profile());
                        (size, align, vec![field(&payload.name, 0, source.clone())])
                    } else {
                        (0, 1, Vec::new())
                    };
                    fields.push(field(
                        &variant.name,
                        0,
                        inline(DebugType {
                            name: format!("{}::{}", definition.name, variant.name),
                            size,
                            align,
                            kind: DebugTypeKind::Record(fields_),
                        }),
                    ));
                }
                DebugTypeKind::Record(fields)
            }
        })
    }
}
