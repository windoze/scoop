use super::diagnostic::verify_descriptor_diagnostic_relocation;
use super::itable::verify_itable_directory;
use super::*;

pub(super) fn verify_descriptor<D, C>(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    objects: &BTreeMap<SlibMemberId, &[u8]>,
    plan: &StrongTypeRegistrationPlan<D, C>,
    plans_by_exact: &BTreeMap<PersistentExactTypeId, &StrongTypeRegistrationPlan<D, C>>,
) -> Result<VerifiedStrongTypeDescriptorV1, StrongTypeRegistrationValidationError>
where
    D: LinkDescriptorReference,
{
    let member = required_scoop_member(
        patch_sites.builtins(),
        plan,
        plan.descriptor_definition_plan(),
    )?;
    let verified = verified_member(patch_sites.builtins(), member)?;
    let definition = verified
        .definitions()
        .definition(plan.descriptor_definition_plan())
        .ok_or(
            StrongTypeRegistrationValidationError::MissingVerifiedDefinition {
                exact_type: plan.exact_type(),
                definition: plan.descriptor_definition_plan(),
            },
        )?;
    if definition.primary_atom() != plan.descriptor_primary_atom() {
        return Err(
            StrongTypeRegistrationValidationError::DescriptorPrimaryAtomMismatch {
                exact_type: plan.exact_type(),
                expected: plan.descriptor_primary_atom(),
                actual: definition.primary_atom(),
            },
        );
    }
    let primary = definition
        .atoms()
        .iter()
        .find(|atom| {
            atom.atom() == plan.descriptor_primary_atom()
                && atom.atom_role() == DefinitionAtomRole::Primary
        })
        .copied()
        .ok_or(
            StrongTypeRegistrationValidationError::MissingDescriptorPrimaryAtom {
                exact_type: plan.exact_type(),
                atom: plan.descriptor_primary_atom(),
            },
        )?;
    let (primary_section, primary_start, primary_end) = atom_file_range(verified, primary)
        .map_err(|kind| {
            StrongTypeRegistrationValidationError::InvalidDescriptorPrimaryAtomFileRange {
                exact_type: plan.exact_type(),
                atom: plan.descriptor_primary_atom(),
                kind,
            }
        })?;
    if primary_section != BuiltinObjectSectionRoleV1::ReadOnlyData {
        return Err(
            StrongTypeRegistrationValidationError::DescriptorPrimaryAtomSectionMismatch {
                exact_type: plan.exact_type(),
            },
        );
    }
    let primary_size = primary_end - primary_start;
    let expected_size = TYPE_DESCRIPTOR_SIZE
        + u64::try_from(plan.semantic().relations().related_types().len())
            .expect("function arity fits u64")
            * 8;
    if primary_size != expected_size {
        return Err(
            StrongTypeRegistrationValidationError::DescriptorPrimaryAtomSizeMismatch {
                exact_type: plan.exact_type(),
                actual: primary_size,
            },
        );
    }

    let diagnostic = definition
        .atoms()
        .iter()
        .find(|atom| {
            atom.atom() == plan.diagnostic_atom()
                && atom.atom_role() == DefinitionAtomRole::AddressTakenConstant
        })
        .copied()
        .ok_or(
            StrongTypeRegistrationValidationError::MissingDescriptorDiagnosticAtom {
                exact_type: plan.exact_type(),
                atom: plan.diagnostic_atom(),
            },
        )?;
    let (diagnostic_section, diagnostic_start, diagnostic_end) =
        atom_file_range(verified, diagnostic).map_err(|kind| {
            StrongTypeRegistrationValidationError::InvalidDescriptorDiagnosticAtomFileRange {
                exact_type: plan.exact_type(),
                atom: plan.diagnostic_atom(),
                kind,
            }
        })?;
    if diagnostic_section != BuiltinObjectSectionRoleV1::ReadOnlyData {
        return Err(
            StrongTypeRegistrationValidationError::DescriptorDiagnosticAtomSectionMismatch {
                exact_type: plan.exact_type(),
            },
        );
    }
    let expected_diagnostic = plan.semantic().diagnostic_name().as_bytes();
    let diagnostic_size = diagnostic_end - diagnostic_start;
    let expected_size =
        u64::try_from(expected_diagnostic.len()).expect("diagnostic length fits u64");
    if diagnostic_size != expected_size {
        return Err(
            StrongTypeRegistrationValidationError::DescriptorDiagnosticAtomSizeMismatch {
                exact_type: plan.exact_type(),
                expected: expected_size,
                actual: diagnostic_size,
            },
        );
    }
    let diagnostic_start_index =
        usize::try_from(diagnostic_start).expect("object offset fits usize");
    let diagnostic_end_index = usize::try_from(diagnostic_end).expect("object offset fits usize");
    let actual_diagnostic = &objects[&member][diagnostic_start_index..diagnostic_end_index];
    if let Some(offset) = actual_diagnostic
        .iter()
        .zip(expected_diagnostic)
        .position(|(actual, expected)| actual != expected)
    {
        return Err(
            StrongTypeRegistrationValidationError::DescriptorDiagnosticByteMismatch {
                exact_type: plan.exact_type(),
                offset_within_atom: u64::try_from(offset).expect("diagnostic offset fits u64"),
                expected: expected_diagnostic[offset],
                actual: actual_diagnostic[offset],
            },
        );
    }

    let diagnostic_relocation = verify_descriptor_diagnostic_relocation(
        verified,
        plan,
        diagnostic.section_ordinal(),
        diagnostic.start(),
    )?;
    let itable_directory = verify_itable_directory(
        patch_sites.builtins(),
        verified,
        definition,
        objects[&member],
        plan,
        plans_by_exact,
    )?;
    Ok(VerifiedStrongTypeDescriptorV1 {
        member,
        primary_symbol_table_index: definition.primary_symbol_table_index(),
        checked_offset: primary_start,
        diagnostic_checked_offset: diagnostic_start,
        diagnostic_size,
        diagnostic_relocation,
        itable_directory,
    })
}
