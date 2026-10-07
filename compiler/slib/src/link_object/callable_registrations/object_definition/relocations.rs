use super::*;

pub(in crate::link_object) fn canonicalize_relocations_with_associated_atoms(
    bytes: &[u8],
    member: &VerifiedMemberObjectRelocationIndexV1,
    atom: scoop_identity::ObjectDefinitionAtomId,
    closure: &VerifiedCurrentConeStrongRelocationClosureV1,
    requirements: &VerifiedObjectDefinitionRequirementSetV1,
    associated_atoms: &[VerifiedDefinitionAtomRangeV1],
) -> Result<Vec<CanonicalObjectRelocationV1>, ObjectDefinitionRelocationFailureV1> {
    let mut output = Vec::new();
    for relocation in member
        .relocations()
        .iter()
        .filter(|relocation| relocation.containing_atom() == atom)
    {
        let bindings = closure.bindings_at(member.member(), atom, relocation.offset_within_atom());
        let mut binding_count = 0;
        let mut targets = Vec::new();
        let mut canonical_value = relocation.encoded_value();
        for (slot, target) in relocation_targets(relocation.shape()) {
            let target = match target {
                VerifiedRelocationTargetV1::LocalDefinition {
                    owner_atom: None,
                    section_ordinal,
                    value,
                    ..
                } if member
                    .definitions()
                    .literal_at(*section_ordinal, *value)
                    .is_some() =>
                {
                    CanonicalRelocationTargetKindV1::Literal(
                        member
                            .definitions()
                            .literal_at(*section_ordinal, *value)
                            .expect("the local target belongs to a literal pool"),
                    )
                }
                VerifiedRelocationTargetV1::LocalDefinition {
                    owner_atom: Some(target_atom),
                    section_ordinal,
                    value,
                    ..
                } => {
                    let range = associated_atoms
                        .iter()
                        .find(|range| range.atom() == *target_atom)
                        .ok_or(
                            ObjectDefinitionRelocationFailureV1::UnsupportedLocalOrSectionTarget,
                        )?;
                    if range.section_ordinal() != *section_ordinal
                        || *value < range.start()
                        || *value >= range.end()
                    {
                        return Err(
                            ObjectDefinitionRelocationFailureV1::UnsupportedLocalOrSectionTarget,
                        );
                    }
                    CanonicalRelocationTargetKindV1::OwningAssociatedAtomOffset {
                        atom: *target_atom,
                        role: range.atom_role(),
                        offset_within_atom: *value - range.start(),
                    }
                }
                VerifiedRelocationTargetV1::SectionBase {
                    section_ordinal, ..
                } if matches!(
                    relocation.shape().form(),
                    VerifiedObjectRelocationFormV1::ElfRela { .. }
                ) =>
                {
                    // Preserve S + A: the atom at the section base supplies S,
                    // and RELA keeps A, including instruction-relative biases.
                    let range = associated_atoms
                        .iter()
                        .find(|range| {
                            range.section_ordinal() == *section_ordinal
                                && range.start() == 0
                                && range.end() != 0
                        })
                        .ok_or(
                            ObjectDefinitionRelocationFailureV1::UnsupportedLocalOrSectionTarget,
                        )?;
                    CanonicalRelocationTargetKindV1::OwningAssociatedAtomOffset {
                        atom: range.atom(),
                        role: range.atom_role(),
                        offset_within_atom: 0,
                    }
                }
                VerifiedRelocationTargetV1::SectionBase {
                    section_ordinal, ..
                } if relocation.shape().form() == VerifiedObjectRelocationFormV1::Unsigned64 => {
                    let address = relocation.encoded_value();
                    let range = associated_atoms
                        .iter()
                        .find(|range| {
                            range.section_ordinal().get() == section_ordinal.get()
                                && range.start() <= address
                                && address < range.end()
                        })
                        .ok_or(
                            ObjectDefinitionRelocationFailureV1::UnsupportedLocalOrSectionTarget,
                        )?;
                    canonical_value = 0;
                    CanonicalRelocationTargetKindV1::OwningAssociatedAtomOffset {
                        atom: range.atom(),
                        role: range.atom_role(),
                        offset_within_atom: address - range.start(),
                    }
                }
                VerifiedRelocationTargetV1::LocalDefinition { .. }
                | VerifiedRelocationTargetV1::SectionBase { .. } => {
                    return Err(
                        ObjectDefinitionRelocationFailureV1::UnsupportedLocalOrSectionTarget,
                    );
                }
                VerifiedRelocationTargetV1::StrongDefinition { .. }
                | VerifiedRelocationTargetV1::ExternalUndefined { .. } => {
                    let binding = bindings
                        .iter()
                        .find(|binding| binding.target_slot() == slot)
                        .ok_or(ObjectDefinitionRelocationFailureV1::BindingCount)?;
                    binding_count += 1;
                    CanonicalRelocationTargetKindV1::Requirement(canonical_requirement(
                        binding,
                        requirements,
                    )?)
                }
            };
            targets.push(CanonicalRelocationTargetV1 { slot, target });
        }
        if bindings.len() != binding_count {
            return Err(ObjectDefinitionRelocationFailureV1::BindingCount);
        }
        targets.sort_unstable_by_key(|target| target.slot);
        output.push(CanonicalObjectRelocationV1 {
            offset_within_atom: relocation.offset_within_atom(),
            form: relocation.shape().form(),
            encoded_value: relocation.encoded_value(),
            canonical_value,
            targets,
        });
    }
    output.sort_unstable_by_key(|relocation| relocation.offset_within_atom);
    for relocation in &output {
        let start = usize::try_from(relocation.offset_within_atom)
            .map_err(|_| ObjectDefinitionRelocationFailureV1::Range)?;
        let width = match relocation.form {
            VerifiedObjectRelocationFormV1::ElfRela { width, .. } => usize::from(width),
            VerifiedObjectRelocationFormV1::Unsigned64
            | VerifiedObjectRelocationFormV1::Subtractor64 => 8,
            _ => 4,
        };
        if bytes.get(start..start + width).is_none() {
            return Err(ObjectDefinitionRelocationFailureV1::Range);
        }
    }
    Ok(output)
}

fn relocation_targets(
    shape: &VerifiedObjectRelocationShapeV1,
) -> Vec<(RelocationTargetSlotV1, &VerifiedRelocationTargetV1)> {
    match shape {
        VerifiedObjectRelocationShapeV1::ElfRela { target, .. }
        | VerifiedObjectRelocationShapeV1::Unsigned64 { target }
        | VerifiedObjectRelocationShapeV1::Branch26 { target }
        | VerifiedObjectRelocationShapeV1::Page21 { target, .. }
        | VerifiedObjectRelocationShapeV1::PageOffset12 { target, .. }
        | VerifiedObjectRelocationShapeV1::GotLoadPage21 { target }
        | VerifiedObjectRelocationShapeV1::GotLoadPageOffset12 { target }
        | VerifiedObjectRelocationShapeV1::PointerToGot32 { target }
        | VerifiedObjectRelocationShapeV1::TlvpLoadPage21 { target }
        | VerifiedObjectRelocationShapeV1::TlvpLoadPageOffset12 { target } => {
            vec![(RelocationTargetSlotV1::Single, target)]
        }
        VerifiedObjectRelocationShapeV1::Subtractor64 {
            minuend,
            subtrahend,
        } => vec![
            (RelocationTargetSlotV1::Minuend, minuend),
            (RelocationTargetSlotV1::Subtrahend, subtrahend),
        ],
    }
}

fn canonical_requirement(
    binding: &crate::link_object::StrongRelocationBindingV1,
    requirements: &VerifiedObjectDefinitionRequirementSetV1,
) -> Result<CanonicalObjectDefinitionRequirementV1, ObjectDefinitionRelocationFailureV1> {
    match binding.resolution() {
        StrongRelocationResolutionV1::ObjectLocalStrong { owner, .. } => match owner {
            LinkDefinitionOwnerV1::OdrDefinition(member) => {
                Ok(CanonicalObjectDefinitionRequirementV1::Legacy(
                    FinalUndefinedSymbolRequirementV1::OdrMember { member },
                ))
            }
            LinkDefinitionOwnerV1::StrongDefinition(owner) => {
                Ok(CanonicalObjectDefinitionRequirementV1::Legacy(
                    FinalUndefinedSymbolRequirementV1::IntraConeStrong { owner },
                ))
            }
            _ => Err(ObjectDefinitionRelocationFailureV1::UnsupportedObjectLocalOwner),
        },
        StrongRelocationResolutionV1::CurrentConeUndefinedStrong { .. }
        | StrongRelocationResolutionV1::ExternalCandidate { .. } => {
            let use_site = CanonicalUndefinedRelocationUseV1::from(binding);
            requirements
                .requirement_for(&use_site)
                .ok_or(ObjectDefinitionRelocationFailureV1::MissingUndefinedRequirement)
        }
    }
}
