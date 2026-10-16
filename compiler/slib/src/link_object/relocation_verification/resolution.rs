//! Resolve native relocation targets against the member symbol plan.

use super::*;

pub(super) fn resolve_shape(
    shape: crate::link_object::ObjectRelocationShapeV1,
    definitions: &MemberObjectDefinitionIndexV1,
    local_symbol_owners: &BTreeMap<u32, ObjectDefinitionAtomId>,
) -> Result<VerifiedObjectRelocationShapeV1, ObjectRelocationValidationError> {
    let shape = match shape {
        crate::link_object::ObjectRelocationShapeV1::DarwinArm64(shape) => shape,
        crate::link_object::ObjectRelocationShapeV1::ElfX86_64Rela {
            kind,
            target_symbol,
            addend,
            width,
        } => {
            let target = resolve_target(
                DarwinArm64RelocationTargetV1::SymbolTableIndex(target_symbol),
                definitions,
                local_symbol_owners,
            )?;
            return Ok(VerifiedObjectRelocationShapeV1::ElfX86_64Rela {
                kind,
                addend,
                width,
                target,
            });
        }
    };
    Ok(match shape {
        DarwinArm64RelocationShapeV1::Unsigned64 { target } => {
            VerifiedObjectRelocationShapeV1::Unsigned64 {
                target: resolve_target(target, definitions, local_symbol_owners)?,
            }
        }
        DarwinArm64RelocationShapeV1::Subtractor64 {
            minuend,
            subtrahend,
        } => VerifiedObjectRelocationShapeV1::Subtractor64 {
            minuend: resolve_target(minuend, definitions, local_symbol_owners)?,
            subtrahend: resolve_target(subtrahend, definitions, local_symbol_owners)?,
        },
        DarwinArm64RelocationShapeV1::Branch26 { target } => {
            VerifiedObjectRelocationShapeV1::Branch26 {
                target: resolve_target(target, definitions, local_symbol_owners)?,
            }
        }
        DarwinArm64RelocationShapeV1::Page21 {
            target,
            explicit_addend,
        } => VerifiedObjectRelocationShapeV1::Page21 {
            target: resolve_target(target, definitions, local_symbol_owners)?,
            explicit_addend,
        },
        DarwinArm64RelocationShapeV1::PageOffset12 {
            target,
            explicit_addend,
        } => VerifiedObjectRelocationShapeV1::PageOffset12 {
            target: resolve_target(target, definitions, local_symbol_owners)?,
            explicit_addend,
        },
        DarwinArm64RelocationShapeV1::GotLoadPage21 { target } => {
            VerifiedObjectRelocationShapeV1::GotLoadPage21 {
                target: resolve_target(target, definitions, local_symbol_owners)?,
            }
        }
        DarwinArm64RelocationShapeV1::GotLoadPageOffset12 { target } => {
            VerifiedObjectRelocationShapeV1::GotLoadPageOffset12 {
                target: resolve_target(target, definitions, local_symbol_owners)?,
            }
        }
        DarwinArm64RelocationShapeV1::PointerToGot32 { target } => {
            VerifiedObjectRelocationShapeV1::PointerToGot32 {
                target: resolve_target(target, definitions, local_symbol_owners)?,
            }
        }
        DarwinArm64RelocationShapeV1::TlvpLoadPage21 { target } => {
            VerifiedObjectRelocationShapeV1::TlvpLoadPage21 {
                target: resolve_target(target, definitions, local_symbol_owners)?,
            }
        }
        DarwinArm64RelocationShapeV1::TlvpLoadPageOffset12 { target } => {
            VerifiedObjectRelocationShapeV1::TlvpLoadPageOffset12 {
                target: resolve_target(target, definitions, local_symbol_owners)?,
            }
        }
    })
}

fn resolve_target(
    target: DarwinArm64RelocationTargetV1,
    definitions: &MemberObjectDefinitionIndexV1,
    local_symbol_owners: &BTreeMap<u32, ObjectDefinitionAtomId>,
) -> Result<VerifiedRelocationTargetV1, ObjectRelocationValidationError> {
    match target {
        DarwinArm64RelocationTargetV1::SectionOrdinal(section_ordinal) => {
            let index = usize::try_from(section_ordinal.get() - 1).map_err(|_| {
                ObjectRelocationValidationError::InvalidSectionTarget {
                    section: section_ordinal,
                }
            })?;
            let section_role = definitions.sections().roles().get(index).copied().ok_or(
                ObjectRelocationValidationError::InvalidSectionTarget {
                    section: section_ordinal,
                },
            )?;
            Ok(VerifiedRelocationTargetV1::SectionBase {
                section_ordinal,
                section_role,
            })
        }
        DarwinArm64RelocationTargetV1::SymbolTableIndex(table_index) => {
            if let Some(strong) = definitions.strong_symbol_by_table_index(table_index) {
                return match strong.role() {
                    PlannedStrongObjectSymbolRoleV1::PrimaryDefinition { definition, .. } => {
                        Ok(VerifiedRelocationTargetV1::StrongDefinition { definition })
                    }
                    PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart { atom, .. } => {
                        let symbol = definitions
                            .sections()
                            .envelope()
                            .symbols()
                            .get(table_index as usize)
                            .ok_or(ObjectRelocationValidationError::InvalidSymbolTarget {
                                table_index,
                            })?;
                        let section_ordinal = symbol.section_ordinal().ok_or(
                            ObjectRelocationValidationError::InvalidSymbolTarget { table_index },
                        )?;
                        Ok(VerifiedRelocationTargetV1::LocalDefinition {
                            table_index,
                            name: symbol.name().to_vec(),
                            owner_atom: Some(atom),
                            section_ordinal,
                            value: symbol.value(),
                        })
                    }
                    PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd { atom, .. } => {
                        Err(ObjectRelocationValidationError::BoundaryRelocationTarget {
                            atom,
                            boundary: VerifiedBoundaryRoleV1::End,
                        })
                    }
                };
            }
            let symbol = definitions
                .sections()
                .envelope()
                .symbols()
                .get(table_index as usize)
                .ok_or(ObjectRelocationValidationError::InvalidSymbolTarget { table_index })?;
            match symbol.kind() {
                ObjectSymbolKindV1::ExternalStrongDefinition
                | ObjectSymbolKindV1::ExternalWeakDefinition => Err(
                    ObjectRelocationValidationError::UnplannedStrongDefinitionTarget {
                        table_index,
                    },
                ),
                ObjectSymbolKindV1::FileMetadata => {
                    Err(ObjectRelocationValidationError::InvalidSymbolTarget { table_index })
                }
                ObjectSymbolKindV1::SectionBase => {
                    let section_ordinal = symbol.section_ordinal().ok_or(
                        ObjectRelocationValidationError::InvalidSymbolTarget { table_index },
                    )?;
                    Ok(VerifiedRelocationTargetV1::SectionBase {
                        section_ordinal,
                        section_role: definitions.sections().roles()
                            [section_ordinal.get() as usize - 1],
                    })
                }
                ObjectSymbolKindV1::LocalSectionDefinition => {
                    let section_ordinal = symbol.section_ordinal().ok_or(
                        ObjectRelocationValidationError::InvalidSymbolTarget { table_index },
                    )?;
                    Ok(VerifiedRelocationTargetV1::LocalDefinition {
                        table_index,
                        name: symbol.name().to_vec(),
                        owner_atom: local_symbol_owners.get(&table_index).copied(),
                        section_ordinal,
                        value: symbol.value(),
                    })
                }
                ObjectSymbolKindV1::ExternalUndefined => {
                    Ok(VerifiedRelocationTargetV1::ExternalUndefined {
                        table_index,
                        name: symbol.name().to_vec(),
                    })
                }
            }
        }
    }
}
