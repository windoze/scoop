//! Resolve native relocation targets against the member symbol plan.

use super::*;

pub(super) fn resolve_shape(
    shape: crate::link_object::ObjectRelocationShapeV1,
    definitions: &MemberObjectDefinitionIndexV1,
    local_symbol_owners: &BTreeMap<u32, ObjectDefinitionAtomId>,
) -> Result<VerifiedObjectRelocationShapeV1, ObjectRelocationValidationError> {
    let (machine, shape) = match shape {
        crate::link_object::ObjectRelocationShapeV1::MachO { machine, shape } => (machine, shape),
        crate::link_object::ObjectRelocationShapeV1::Coff {
            machine,
            kind,
            target_symbol,
            storage_width,
        } => {
            let target = resolve_target(
                ObjectRelocationTargetV1::SymbolTableIndex(target_symbol),
                definitions,
                local_symbol_owners,
            )?;
            return Ok(VerifiedObjectRelocationShapeV1::Coff {
                machine,
                kind,
                storage_width,
                target,
            });
        }
        crate::link_object::ObjectRelocationShapeV1::ElfRela {
            machine,
            kind,
            target_symbol,
            addend,
            storage_width,
        } => {
            let target = resolve_target(
                ObjectRelocationTargetV1::SymbolTableIndex(target_symbol),
                definitions,
                local_symbol_owners,
            )?;
            return Ok(VerifiedObjectRelocationShapeV1::ElfRela {
                machine,
                kind,
                addend,
                storage_width,
                target,
            });
        }
    };
    let shape = match shape {
        MachORelocationShapeV1::Unsigned32 { target } => {
            VerifiedMachORelocationShapeV1::Unsigned32 {
                target: resolve_target(target, definitions, local_symbol_owners)?,
            }
        }
        MachORelocationShapeV1::Subtractor32 {
            minuend,
            subtrahend,
        } => VerifiedMachORelocationShapeV1::Subtractor32 {
            minuend: resolve_target(minuend, definitions, local_symbol_owners)?,
            subtrahend: resolve_target(subtrahend, definitions, local_symbol_owners)?,
        },
        MachORelocationShapeV1::X86_64PcRelative32 { kind, target } => {
            VerifiedMachORelocationShapeV1::X86_64PcRelative32 {
                kind,
                target: resolve_target(target, definitions, local_symbol_owners)?,
            }
        }
        MachORelocationShapeV1::Unsigned64 { target } => {
            VerifiedMachORelocationShapeV1::Unsigned64 {
                target: resolve_target(target, definitions, local_symbol_owners)?,
            }
        }
        MachORelocationShapeV1::Subtractor64 {
            minuend,
            subtrahend,
        } => VerifiedMachORelocationShapeV1::Subtractor64 {
            minuend: resolve_target(minuend, definitions, local_symbol_owners)?,
            subtrahend: resolve_target(subtrahend, definitions, local_symbol_owners)?,
        },
        MachORelocationShapeV1::Branch26 { target } => VerifiedMachORelocationShapeV1::Branch26 {
            target: resolve_target(target, definitions, local_symbol_owners)?,
        },
        MachORelocationShapeV1::Page21 {
            target,
            explicit_addend,
        } => VerifiedMachORelocationShapeV1::Page21 {
            target: resolve_target(target, definitions, local_symbol_owners)?,
            explicit_addend,
        },
        MachORelocationShapeV1::PageOffset12 {
            target,
            explicit_addend,
        } => VerifiedMachORelocationShapeV1::PageOffset12 {
            target: resolve_target(target, definitions, local_symbol_owners)?,
            explicit_addend,
        },
        MachORelocationShapeV1::GotLoadPage21 { target } => {
            VerifiedMachORelocationShapeV1::GotLoadPage21 {
                target: resolve_target(target, definitions, local_symbol_owners)?,
            }
        }
        MachORelocationShapeV1::GotLoadPageOffset12 { target } => {
            VerifiedMachORelocationShapeV1::GotLoadPageOffset12 {
                target: resolve_target(target, definitions, local_symbol_owners)?,
            }
        }
        MachORelocationShapeV1::PointerToGot32 { target } => {
            VerifiedMachORelocationShapeV1::PointerToGot32 {
                target: resolve_target(target, definitions, local_symbol_owners)?,
            }
        }
        MachORelocationShapeV1::TlvpLoadPage21 { target } => {
            VerifiedMachORelocationShapeV1::TlvpLoadPage21 {
                target: resolve_target(target, definitions, local_symbol_owners)?,
            }
        }
        MachORelocationShapeV1::TlvpLoadPageOffset12 { target } => {
            VerifiedMachORelocationShapeV1::TlvpLoadPageOffset12 {
                target: resolve_target(target, definitions, local_symbol_owners)?,
            }
        }
    };
    Ok(VerifiedObjectRelocationShapeV1::MachO { machine, shape })
}

fn resolve_target(
    target: ObjectRelocationTargetV1,
    definitions: &MemberObjectDefinitionIndexV1,
    local_symbol_owners: &BTreeMap<u32, ObjectDefinitionAtomId>,
) -> Result<VerifiedRelocationTargetV1, ObjectRelocationValidationError> {
    match target {
        ObjectRelocationTargetV1::SectionOrdinal(section_ordinal) => {
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
        ObjectRelocationTargetV1::SymbolTableIndex(table_index) => {
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
            // LLVM may give ODR profile data weak external linkage. It is an
            // object-local native target, not an additional Scoop definition.
            if let Some(section_ordinal) = symbol.section_ordinal()
                && definitions.sections().roles()[section_ordinal.get() as usize - 1]
                    == BuiltinObjectSectionRoleV1::ProfilingSupport
            {
                return Ok(VerifiedRelocationTargetV1::LocalDefinition {
                    table_index,
                    name: symbol.name().to_vec(),
                    owner_atom: definitions
                        .definitions()
                        .iter()
                        .flat_map(|definition| definition.atoms())
                        .find(|atom| {
                            atom.section_ordinal() == section_ordinal
                                && atom.start() <= symbol.value()
                                && symbol.value() < atom.end()
                        })
                        .map(|atom| atom.atom()),
                    section_ordinal,
                    value: symbol.value(),
                });
            }
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
