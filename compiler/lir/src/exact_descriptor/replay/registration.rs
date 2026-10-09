use super::*;

pub(crate) fn validate_registration_plan(
    registration: &StrongTypeRegistrationPlanV2,
    layouts: &crate::CanonicalExactLayoutExportsV1,
    foundation: &ConeLirFoundation,
) -> Result<(), ExactDescriptorError> {
    use scoop_identity::{DefinitionAtomRole, DefinitionAtomSubkey, ObjectDefinitionAtomKey};

    let exact = registration.exact_type();
    let descriptor = StrongShapeDefinitionRefV1::from_foundation(
        ExternalStrongShapeSubjectV1::TypeDescriptor(exact),
        foundation,
    )?;
    if registration.descriptor_definition_plan() != descriptor.definition()
        || registration.descriptor_primary_atom() != descriptor.primary()
        || registration.descriptor_symbol() != descriptor.symbol()
    {
        return Err(ExactDescriptorError::DescriptorPlan(exact));
    }
    let instance = layouts
        .find_exact_role(exact, RepresentationRole::ManagedObject)
        .ok_or(ExactDescriptorError::MissingInstanceLayout(exact))?;
    let layout = instance.identity().physical_definition();
    if registration.layout() != instance.identity().layout()
        || registration.layout_definition_plan() != layout.definition()
        || registration.layout_primary_atom() != layout.primary()
        || registration.layout_symbol() != layout.symbol()
    {
        return Err(ExactDescriptorError::LayoutPlan(exact));
    }
    let expected_inline = match registration.semantic().inline_scan() {
        TypeDescriptorInlineScanV1::Null => StrongTypeDescriptorInlineScanPlanV1::Null,
        TypeDescriptorInlineScanV1::Defined(scan) => {
            let physical = StrongShapeDefinitionRefV1::from_foundation(
                ExternalStrongShapeSubjectV1::Scan(scan),
                foundation,
            )?;
            StrongTypeDescriptorInlineScanPlanV1::Defined {
                scan,
                definition_plan: physical.definition(),
            }
        }
    };
    if registration.inline_scan() != expected_inline {
        return Err(ExactDescriptorError::InlineScanPlan(exact));
    }
    let diagnostic = ObjectDefinitionAtomKey::new(
        descriptor.definition(),
        DefinitionAtomRole::AddressTakenConstant,
        DefinitionAtomSubkey::ExactType(exact),
    );

    let diagnostic = foundation
        .definition_atoms_for_plan(descriptor.definition())
        .find(|record| record.key() == &diagnostic)
        .map(|record| record.id());
    if diagnostic != Some(registration.diagnostic_atom()) {
        return Err(ExactDescriptorError::DiagnosticAtom(exact));
    }
    let directory = if registration.semantic().itables().is_empty() {
        TypeDescriptorITableDirectoryV1::Null
    } else {
        let key = ObjectDefinitionAtomKey::new(
            descriptor.definition(),
            DefinitionAtomRole::RuntimeRecord,
            DefinitionAtomSubkey::ExactType(exact),
        );

        let atom = foundation
            .definition_atoms_for_plan(descriptor.definition())
            .find(|record| record.key() == &key)
            .map(|record| record.id());
        match atom {
            Some(atom) => TypeDescriptorITableDirectoryV1::Defined(atom),
            None => return Err(ExactDescriptorError::ItableDirectory(exact)),
        }
    };
    if registration.itable_directory() != directory {
        return Err(ExactDescriptorError::ItableDirectory(exact));
    }
    Ok(())
}
