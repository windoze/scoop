use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::{
    DefinitionAtomRole, DefinitionAtomSubkey, GeneratedBridgeAtomId, GeneratedBridgeAtomRoleKey,
    GeneratedBridgeUnitId, GeneratedBridgeUnitKey, ObjectDefinitionAtomId, ObjectDefinitionAtomKey,
    ObjectDefinitionPlanId, ObjectDefinitionPlanKey, StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_lir::{GeneratedBridgeAtomAuthorityRecordV1, GeneratedBridgePlanSetV1};

use super::GeneratedCBridgeSemanticValidationError;
use super::classification::ObservedUnitSemantics;
use crate::{
    SlibMemberId, VerifiedBuiltinObjectStrongRelocationSetV1, VerifiedDarwinArm64RelocationShapeV1,
    VerifiedRelocationTargetV1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum GeneratedBridgeAtomMaterializationRole {
    Primary,
    Associated,
}

#[derive(Clone, Copy)]
pub(super) struct ExpectedBridgeAtom {
    pub(super) unit: GeneratedBridgeUnitId,
    pub(super) member: SlibMemberId,
    pub(super) role: GeneratedBridgeAtomMaterializationRole,
}

pub(super) struct ExpectedBridgeUnit {
    pub(super) unit: GeneratedBridgeUnitId,
    pub(super) producer: scoop_identity::ConeIdentity,
    pub(super) key: GeneratedBridgeUnitKey,
    pub(super) signature_descriptor: Option<(GeneratedBridgeAtomId, ObjectDefinitionPlanId)>,
}

pub(super) struct ExpectedBridgeSet {
    pub(super) units: BTreeMap<GeneratedBridgeUnitId, ExpectedBridgeUnit>,
    pub(super) atoms: BTreeMap<ObjectDefinitionAtomId, ExpectedBridgeAtom>,
    definitions_by_member:
        BTreeMap<SlibMemberId, BTreeMap<ObjectDefinitionPlanId, ObjectDefinitionAtomId>>,
    generated_members: BTreeSet<SlibMemberId>,
}

impl ExpectedBridgeSet {
    pub(super) fn new(
        builtins: &VerifiedBuiltinObjectStrongRelocationSetV1,
        bridge_plan: &GeneratedBridgePlanSetV1,
    ) -> Result<Self, GeneratedCBridgeSemanticValidationError> {
        let mut units = BTreeMap::new();
        let mut atoms = BTreeMap::new();
        let mut definitions_by_member = BTreeMap::<SlibMemberId, BTreeMap<_, _>>::new();
        let generated_members = builtins
            .member_plan()
            .generated_bridge_members()
            .iter()
            .map(|member| member.member_id())
            .collect::<BTreeSet<_>>();
        for plan in bridge_plan.units() {
            let unit = plan.unit();
            let member = builtins
                .member_plan()
                .member_for_generated_bridge_unit(unit)
                .ok_or(GeneratedCBridgeSemanticValidationError::MissingUnitMember(
                    unit,
                ))?;
            let primary = derive_bridge_definition(plan.primary_atom_authority())?;
            register_expected_atom(
                &mut atoms,
                &mut definitions_by_member,
                unit,
                member,
                primary,
                GeneratedBridgeAtomMaterializationRole::Primary,
            )?;
            let mut signature_descriptor = None;
            for atom in plan.materialized_associated_atom_authorities() {
                let definition = derive_bridge_definition(atom)?;
                register_expected_atom(
                    &mut atoms,
                    &mut definitions_by_member,
                    unit,
                    member,
                    definition,
                    GeneratedBridgeAtomMaterializationRole::Associated,
                )?;
                if let GeneratedBridgeAtomRoleKey::SignatureDescriptor { signature, .. } =
                    atom.key().atom()
                {
                    if signature_descriptor
                        .replace((atom.id(), definition.0))
                        .is_some()
                    {
                        return Err(
                            GeneratedCBridgeSemanticValidationError::MultipleSignatureDescriptors(
                                unit,
                            ),
                        );
                    }
                    let GeneratedBridgeUnitKey::CallbackTrampoline {
                        signature: expected,
                        ..
                    } = *plan.unit_authority().key()
                    else {
                        return Err(
                            GeneratedCBridgeSemanticValidationError::UnexpectedSignatureDescriptor(
                                unit,
                            ),
                        );
                    };
                    if signature != expected {
                        return Err(
                            GeneratedCBridgeSemanticValidationError::SignatureDescriptorMismatch(
                                unit,
                            ),
                        );
                    }
                }
            }
            if matches!(
                plan.unit_authority().key(),
                GeneratedBridgeUnitKey::CallbackTrampoline { .. }
            ) && signature_descriptor.is_none()
            {
                return Err(
                    GeneratedCBridgeSemanticValidationError::MissingSignatureDescriptor(unit),
                );
            }
            units.insert(
                unit,
                ExpectedBridgeUnit {
                    unit,
                    producer: bridge_plan.producer(),
                    key: *plan.unit_authority().key(),
                    signature_descriptor,
                },
            );
        }
        Ok(Self {
            units,
            atoms,
            definitions_by_member,
            generated_members,
        })
    }

    pub(super) fn validate_actual_definitions(
        &self,
        builtins: &VerifiedBuiltinObjectStrongRelocationSetV1,
    ) -> Result<(), GeneratedCBridgeSemanticValidationError> {
        for member in builtins.strong_relocations().members() {
            if !self.generated_members.contains(&member.member()) {
                continue;
            }
            let expected = self
                .definitions_by_member
                .get(&member.member())
                .cloned()
                .unwrap_or_default();
            let actual = member
                .definitions()
                .definitions()
                .iter()
                .map(|definition| (definition.definition(), definition))
                .collect::<BTreeMap<_, _>>();
            if actual.keys().copied().collect::<Vec<_>>()
                != expected.keys().copied().collect::<Vec<_>>()
            {
                return Err(
                    GeneratedCBridgeSemanticValidationError::DefinitionCoverageMismatch {
                        member: member.member(),
                    },
                );
            }
            for (definition, atom) in expected {
                let actual = actual.get(&definition).ok_or(
                    GeneratedCBridgeSemanticValidationError::MissingActualDefinition {
                        member: member.member(),
                        definition,
                    },
                )?;
                let [actual_atom] = actual.atoms() else {
                    return Err(
                        GeneratedCBridgeSemanticValidationError::DefinitionAtomShapeMismatch {
                            member: member.member(),
                            definition,
                        },
                    );
                };
                if actual.primary_atom() != atom
                    || actual_atom.atom() != atom
                    || actual_atom.atom_role() != DefinitionAtomRole::Primary
                {
                    return Err(
                        GeneratedCBridgeSemanticValidationError::DefinitionAtomShapeMismatch {
                            member: member.member(),
                            definition,
                        },
                    );
                }
            }
        }
        Ok(())
    }

    pub(super) fn validate_machine_local_relocations(
        &self,
        builtins: &VerifiedBuiltinObjectStrongRelocationSetV1,
    ) -> Result<(), GeneratedCBridgeSemanticValidationError> {
        for member in builtins.strong_relocations().members() {
            if !self.generated_members.contains(&member.member()) {
                continue;
            }
            for relocation in member.relocations() {
                let Some(source) = self.atoms.get(&relocation.containing_atom()) else {
                    return Err(
                        GeneratedCBridgeSemanticValidationError::UnexpectedBridgeRelocationAtom {
                            member: member.member(),
                            atom: relocation.containing_atom(),
                        },
                    );
                };
                for target in relocation_targets(relocation.shape()) {
                    match target {
                        VerifiedRelocationTargetV1::LocalDefinition {
                            owner_atom: Some(owner_atom),
                            ..
                        } if *owner_atom == relocation.containing_atom() => {}
                        VerifiedRelocationTargetV1::LocalDefinition {
                            owner_atom: Some(owner_atom),
                            ..
                        } => {
                            return Err(
                                GeneratedCBridgeSemanticValidationError::CrossAtomLocalRelocation {
                                    unit: source.unit,
                                    source: relocation.containing_atom(),
                                    target: *owner_atom,
                                },
                            );
                        }
                        VerifiedRelocationTargetV1::LocalDefinition {
                            owner_atom: None,
                            table_index,
                            ..
                        } => {
                            return Err(
                                GeneratedCBridgeSemanticValidationError::UnownedLocalRelocation {
                                    unit: source.unit,
                                    source: relocation.containing_atom(),
                                    table_index: *table_index,
                                },
                            );
                        }
                        VerifiedRelocationTargetV1::SectionBase { .. } => {
                            return Err(
                                GeneratedCBridgeSemanticValidationError::SectionBaseRelocation {
                                    unit: source.unit,
                                    atom: relocation.containing_atom(),
                                },
                            );
                        }
                        VerifiedRelocationTargetV1::StrongDefinition { .. }
                        | VerifiedRelocationTargetV1::ExternalUndefined { .. } => {}
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) fn initial_state(&self) -> BTreeMap<GeneratedBridgeUnitId, ObservedUnitSemantics> {
        self.units
            .keys()
            .copied()
            .map(|unit| (unit, ObservedUnitSemantics::default()))
            .collect()
    }
}

type DerivedBridgeDefinition = (ObjectDefinitionPlanId, ObjectDefinitionAtomId);

pub(super) fn derive_bridge_definition(
    atom: &GeneratedBridgeAtomAuthorityRecordV1,
) -> Result<DerivedBridgeDefinition, GeneratedCBridgeSemanticValidationError> {
    let entity = StrongDefinitionEntity::generated_bridge_atom(atom.key()).map_err(|_| {
        GeneratedCBridgeSemanticValidationError::InvalidBridgeAtomIdentity(atom.id())
    })?;
    let key = ObjectDefinitionPlanKey::strong(
        atom.key().producer(),
        entity,
        StrongDefinitionRole::GeneratedBridge,
    )
    .map_err(|_| GeneratedCBridgeSemanticValidationError::InvalidBridgeAtomIdentity(atom.id()))?;
    let definition = ObjectDefinitionPlanId::from_key(&key).map_err(|_| {
        GeneratedCBridgeSemanticValidationError::InvalidBridgeAtomIdentity(atom.id())
    })?;
    let atom = ObjectDefinitionAtomId::from_key(&ObjectDefinitionAtomKey::new(
        definition,
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    ))
    .map_err(|_| GeneratedCBridgeSemanticValidationError::InvalidObjectAtomIdentity(definition))?;
    Ok((definition, atom))
}

fn register_expected_atom(
    atoms: &mut BTreeMap<ObjectDefinitionAtomId, ExpectedBridgeAtom>,
    definitions: &mut BTreeMap<
        SlibMemberId,
        BTreeMap<ObjectDefinitionPlanId, ObjectDefinitionAtomId>,
    >,
    unit: GeneratedBridgeUnitId,
    member: SlibMemberId,
    (definition, atom): DerivedBridgeDefinition,
    role: GeneratedBridgeAtomMaterializationRole,
) -> Result<(), GeneratedCBridgeSemanticValidationError> {
    if atoms
        .insert(atom, ExpectedBridgeAtom { unit, member, role })
        .is_some()
        || definitions
            .entry(member)
            .or_default()
            .insert(definition, atom)
            .is_some()
    {
        return Err(GeneratedCBridgeSemanticValidationError::DuplicateBridgeDefinition(definition));
    }
    Ok(())
}

fn relocation_targets(
    shape: &VerifiedDarwinArm64RelocationShapeV1,
) -> Vec<&VerifiedRelocationTargetV1> {
    match shape {
        VerifiedDarwinArm64RelocationShapeV1::Unsigned64 { target }
        | VerifiedDarwinArm64RelocationShapeV1::Branch26 { target }
        | VerifiedDarwinArm64RelocationShapeV1::Page21 { target, .. }
        | VerifiedDarwinArm64RelocationShapeV1::PageOffset12 { target, .. }
        | VerifiedDarwinArm64RelocationShapeV1::GotLoadPage21 { target }
        | VerifiedDarwinArm64RelocationShapeV1::GotLoadPageOffset12 { target }
        | VerifiedDarwinArm64RelocationShapeV1::PointerToGot32 { target }
        | VerifiedDarwinArm64RelocationShapeV1::TlvpLoadPage21 { target }
        | VerifiedDarwinArm64RelocationShapeV1::TlvpLoadPageOffset12 { target } => vec![target],
        VerifiedDarwinArm64RelocationShapeV1::Subtractor64 {
            minuend,
            subtrahend,
        } => vec![minuend, subtrahend],
    }
}
