use super::*;

pub(super) fn require_descriptor_associated_atoms(
    foundation: &ConeLirFoundation,
    plan: ObjectDefinitionPlanId,
    exact_type: PersistentExactTypeId,
    has_itable_directory: bool,
) -> Result<
    (ObjectDefinitionAtomId, TypeDescriptorITableDirectoryV1),
    StrongTypeRegistrationPlanBuildError,
> {
    let diagnostic = ObjectDefinitionAtomKey::new(
        plan,
        DefinitionAtomRole::AddressTakenConstant,
        DefinitionAtomSubkey::ExactType(exact_type),
    );
    let directory = ObjectDefinitionAtomKey::new(
        plan,
        DefinitionAtomRole::RuntimeRecord,
        DefinitionAtomSubkey::ExactType(exact_type),
    );
    let associated = foundation
        .definition_atoms()
        .iter()
        .filter(|atom| {
            atom.key().plan() == plan && atom.key().role() != DefinitionAtomRole::Primary
        })
        .collect::<Vec<_>>();
    let expected = if has_itable_directory {
        vec![diagnostic.clone(), directory.clone()]
    } else {
        vec![diagnostic.clone()]
    };
    let diagnostic_atom = associated.iter().find(|record| record.key() == &diagnostic);
    let directory_atom = associated.iter().find(|record| record.key() == &directory);
    if associated.len() != expected.len() {
        return Err(
            StrongTypeRegistrationPlanBuildError::DescriptorAssociatedAtomSet {
                exact_type,
                expected,
                actual: associated.iter().map(|atom| atom.id()).collect(),
            },
        );
    }
    let (diagnostic_atom, itable_directory) =
        match (diagnostic_atom, directory_atom, has_itable_directory) {
            (Some(diagnostic), Some(directory), true) => (
                diagnostic.id(),
                TypeDescriptorITableDirectoryV1::Defined(directory.id()),
            ),
            (Some(diagnostic), None, false) => {
                (diagnostic.id(), TypeDescriptorITableDirectoryV1::Null)
            }
            _ => {
                return Err(
                    StrongTypeRegistrationPlanBuildError::DescriptorAssociatedAtomSet {
                        exact_type,
                        expected,
                        actual: associated.iter().map(|atom| atom.id()).collect(),
                    },
                );
            }
        };
    Ok((diagnostic_atom, itable_directory))
}

pub(super) fn require_definition(
    foundation: &ConeLirFoundation,
    entity: StrongDefinitionEntity,
    role: StrongDefinitionRole,
) -> Result<
    &scoop_identity::CborIdentityRecord<ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    StrongTypeRegistrationPlanBuildError,
> {
    foundation
        .definition_for(entity, role)
        .ok_or(StrongTypeRegistrationPlanBuildError::MissingDefinition { entity, role })
}

pub(super) fn require_primary_atom(
    foundation: &ConeLirFoundation,
    plan: ObjectDefinitionPlanId,
) -> Result<ObjectDefinitionAtomId, StrongTypeRegistrationPlanBuildError> {
    let actual = foundation
        .definition_atoms()
        .iter()
        .filter(|atom| {
            atom.key().plan() == plan && atom.key().role() == DefinitionAtomRole::Primary
        })
        .map(|atom| atom.id())
        .collect::<Vec<_>>();
    match actual.as_slice() {
        [atom] => Ok(*atom),
        _ => Err(StrongTypeRegistrationPlanBuildError::PrimaryAtomSet { plan, actual }),
    }
}

pub(super) fn require_symbol(
    foundation: &ConeLirFoundation,
    key: PersistentSymbolKey,
    definition: &scoop_identity::CborIdentityRecord<
        ObjectDefinitionPlanId,
        ObjectDefinitionPlanKey,
    >,
) -> Result<PersistentSymbolRequest, StrongTypeRegistrationPlanBuildError> {
    let linkage = match definition.key().owner() {
        ObjectDefinitionPlanOwner::Strong { .. } => LinkageClass::ConeStrong,
        ObjectDefinitionPlanOwner::Odr { .. } => LinkageClass::OdrWeak,
    };
    let request = PersistentSymbolRequest::new(key, linkage)
        .map_err(StrongTypeRegistrationPlanBuildError::Symbol)?;
    foundation
        .contains_symbol_request(request)
        .then_some(request)
        .ok_or(StrongTypeRegistrationPlanBuildError::MissingSymbol(request))
}

pub(super) fn require_digest_node(
    digests: &DigestFinalizationPlanV1,
    key: DigestNodeKey,
) -> Result<&DigestNodeV1, StrongTypeRegistrationPlanBuildError> {
    digests
        .nodes()
        .iter()
        .find(|node| node.key() == &key)
        .ok_or(StrongTypeRegistrationPlanBuildError::MissingDigestNode(key))
}

pub(super) fn require_patch(
    node: &DigestNodeV1,
    expected: DigestPatchIntentKey,
) -> Result<DigestPatchIntentId, StrongTypeRegistrationPlanBuildError> {
    node.patch_intents()
        .iter()
        .find(|patch| patch.key() == &expected)
        .map(|patch| patch.id())
        .ok_or(StrongTypeRegistrationPlanBuildError::MissingPatch {
            node: node.id(),
            expected: Box::new(expected),
        })
}
