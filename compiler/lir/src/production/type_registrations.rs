//! Complete writer-side production plans for strong type registrations.

use std::fmt;

pub use scoop_identity::PersistentExactTypeId;
use scoop_identity::{
    ConeIdentity, DefinitionAtomRole, DefinitionAtomSubkey, DigestNodeId, DigestNodeKey,
    DigestPatchIntentId, DigestPatchIntentKey, DigestSemanticFieldRole, LinkageClass,
    ObjectDefinitionAtomId, ObjectDefinitionAtomKey, ObjectDefinitionIdentityError,
    ObjectDefinitionPlanId, ObjectDefinitionPlanKey, ObjectDefinitionPlanOwner,
    ObjectDefinitionPlanRole, PersistentLayoutId, PersistentSymbolError, PersistentSymbolKey,
    PersistentSymbolRequest, RepresentationRole, RuntimeTypeId, StrongDefinitionEntity,
    StrongDefinitionEntityKind, StrongDefinitionRole, TargetProfileWireId,
};

use crate::{
    DigestInputRefV1, DigestNodeV1, OdrFreeLirFoundation, StrongDigestFinalizationPlanV1,
    StrongRegistrationIdentitySurfaceV1,
};

mod semantics;
pub use semantics::*;

/// All semantic identities and graph writers required to emit one strong
/// type-registration record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongTypeRegistrationPlanV1 {
    semantic: StrongTypeDescriptorSemanticPlanV1,
    runtime_type: RuntimeTypeId,
    symbol: PersistentSymbolRequest,
    definition_plan: ObjectDefinitionPlanId,
    primary_atom: ObjectDefinitionAtomId,
    descriptor_symbol: PersistentSymbolRequest,
    descriptor_definition_plan: ObjectDefinitionPlanId,
    descriptor_primary_atom: ObjectDefinitionAtomId,
    diagnostic_atom: ObjectDefinitionAtomId,
    layout_symbol: PersistentSymbolRequest,
    layout_definition_plan: ObjectDefinitionPlanId,
    layout_primary_atom: ObjectDefinitionAtomId,
    registration_object_node: DigestNodeId,
    descriptor_definition_node: DigestNodeId,
    layout_fingerprint_node: DigestNodeId,
    registration_fingerprint_node: DigestNodeId,
    registration_definition_patch: DigestPatchIntentId,
    descriptor_definition_patch: DigestPatchIntentId,
    layout_fingerprint_patch: DigestPatchIntentId,
}

impl StrongTypeRegistrationPlanV1 {
    pub const fn semantic(&self) -> &StrongTypeDescriptorSemanticPlanV1 {
        &self.semantic
    }

    pub const fn exact_type(&self) -> PersistentExactTypeId {
        self.semantic.exact_type()
    }

    pub const fn runtime_type(&self) -> RuntimeTypeId {
        self.runtime_type
    }

    pub const fn symbol(&self) -> PersistentSymbolRequest {
        self.symbol
    }

    pub const fn definition_plan(&self) -> ObjectDefinitionPlanId {
        self.definition_plan
    }

    pub const fn primary_atom(&self) -> ObjectDefinitionAtomId {
        self.primary_atom
    }

    pub const fn descriptor_symbol(&self) -> PersistentSymbolRequest {
        self.descriptor_symbol
    }

    pub const fn descriptor_definition_plan(&self) -> ObjectDefinitionPlanId {
        self.descriptor_definition_plan
    }

    pub const fn descriptor_primary_atom(&self) -> ObjectDefinitionAtomId {
        self.descriptor_primary_atom
    }

    pub const fn diagnostic_atom(&self) -> ObjectDefinitionAtomId {
        self.diagnostic_atom
    }

    pub const fn layout(&self) -> PersistentLayoutId {
        self.semantic.instance_layout()
    }

    pub const fn layout_symbol(&self) -> PersistentSymbolRequest {
        self.layout_symbol
    }

    pub const fn layout_definition_plan(&self) -> ObjectDefinitionPlanId {
        self.layout_definition_plan
    }

    pub const fn layout_primary_atom(&self) -> ObjectDefinitionAtomId {
        self.layout_primary_atom
    }

    pub const fn registration_object_node(&self) -> DigestNodeId {
        self.registration_object_node
    }

    pub const fn descriptor_definition_node(&self) -> DigestNodeId {
        self.descriptor_definition_node
    }

    pub const fn layout_fingerprint_node(&self) -> DigestNodeId {
        self.layout_fingerprint_node
    }

    pub const fn registration_fingerprint_node(&self) -> DigestNodeId {
        self.registration_fingerprint_node
    }

    pub const fn registration_definition_patch(&self) -> DigestPatchIntentId {
        self.registration_definition_patch
    }

    pub const fn descriptor_definition_patch(&self) -> DigestPatchIntentId {
        self.descriptor_definition_patch
    }

    pub const fn layout_fingerprint_patch(&self) -> DigestPatchIntentId {
        self.layout_fingerprint_patch
    }
}

/// Proof that every locally materialized type descriptor has exactly one
/// complete strong type-registration production plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongTypeRegistrationPlanSetV1 {
    producer: ConeIdentity,
    target: TargetProfileWireId,
    registrations: Vec<StrongTypeRegistrationPlanV1>,
}

impl StrongTypeRegistrationPlanSetV1 {
    pub fn new(
        target: crate::LirTargetProfile,
        foundation: &OdrFreeLirFoundation,
        identities: &StrongRegistrationIdentitySurfaceV1,
        semantics: &StrongTypeDescriptorSemanticPlanSetV1,
        digests: &StrongDigestFinalizationPlanV1,
    ) -> Result<Self, StrongTypeRegistrationPlanBuildError> {
        if semantics.producer() != foundation.producer() {
            return Err(StrongTypeRegistrationPlanBuildError::ProducerMismatch {
                foundation: foundation.producer(),
                semantics: semantics.producer(),
            });
        }
        if semantics.target() != &target.wire_id() {
            return Err(StrongTypeRegistrationPlanBuildError::TargetMismatch {
                expected: target.wire_id(),
                actual: semantics.target().clone(),
            });
        }
        let mut expected = foundation
            .definition_plans()
            .iter()
            .filter_map(
                |record| match (record.key().owner(), record.key().definition_role()) {
                    (
                        ObjectDefinitionPlanOwner::Strong { entity, .. },
                        ObjectDefinitionPlanRole::Strong(StrongDefinitionRole::TypeDescriptor),
                    ) => match entity.kind() {
                        StrongDefinitionEntityKind::ExactType(exact) => Some(exact),
                        _ => None,
                    },
                    _ => None,
                },
            )
            .collect::<Vec<_>>();
        expected.sort_unstable();
        if let Some(pair) = expected.windows(2).find(|pair| pair[0] == pair[1]) {
            return Err(StrongTypeRegistrationPlanBuildError::DuplicateDescriptor(
                pair[0],
            ));
        }
        let semantic_types = semantics
            .descriptors()
            .iter()
            .map(StrongTypeDescriptorSemanticPlanV1::exact_type)
            .collect::<Vec<_>>();
        if expected != semantic_types {
            return Err(
                StrongTypeRegistrationPlanBuildError::DescriptorSemanticSet {
                    expected,
                    actual: semantic_types,
                },
            );
        }
        let actual = identities
            .type_registrations()
            .iter()
            .map(|registration| registration.semantic_id())
            .collect::<Vec<_>>();
        if semantic_types != actual {
            return Err(StrongTypeRegistrationPlanBuildError::TypeSet {
                expected: semantic_types,
                actual,
            });
        }

        let registrations = semantics
            .descriptors()
            .iter()
            .zip(identities.type_registrations())
            .map(|(semantic, identity)| {
                build_registration(target, foundation, semantic, identity, digests)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            producer: foundation.producer(),
            target: target.wire_id(),
            registrations,
        })
    }

    pub const fn producer(&self) -> ConeIdentity {
        self.producer
    }

    pub const fn target(&self) -> &TargetProfileWireId {
        &self.target
    }

    pub fn registrations(&self) -> &[StrongTypeRegistrationPlanV1] {
        &self.registrations
    }
}

fn build_registration(
    target: crate::LirTargetProfile,
    foundation: &OdrFreeLirFoundation,
    semantic: &StrongTypeDescriptorSemanticPlanV1,
    identity: &crate::StrongRegistrationIdentityV1<PersistentExactTypeId>,
    digests: &StrongDigestFinalizationPlanV1,
) -> Result<StrongTypeRegistrationPlanV1, StrongTypeRegistrationPlanBuildError> {
    let exact_type = semantic.exact_type();
    let registration_definition = require_definition(
        foundation,
        StrongDefinitionEntity::exact_type(exact_type),
        StrongDefinitionRole::TypeRegistration,
    )?;
    if registration_definition.id() != identity.definition_plan() {
        return Err(
            StrongTypeRegistrationPlanBuildError::RegistrationDefinitionMismatch {
                exact_type,
                expected: registration_definition.id(),
                actual: identity.definition_plan(),
            },
        );
    }
    let primary_atom = require_primary_atom(foundation, registration_definition.id())?;
    let symbol = require_symbol(
        foundation,
        PersistentSymbolKey::TypeRegistration(exact_type),
    )?;

    let descriptor_definition = require_definition(
        foundation,
        StrongDefinitionEntity::exact_type(exact_type),
        StrongDefinitionRole::TypeDescriptor,
    )?;
    let descriptor_primary_atom = require_primary_atom(foundation, descriptor_definition.id())?;
    let diagnostic_atom =
        require_descriptor_diagnostic_atom(foundation, descriptor_definition.id(), exact_type)?;
    let descriptor_symbol =
        require_symbol(foundation, PersistentSymbolKey::TypeDescriptor(exact_type))?;

    let runtime_types = foundation
        .runtime_types()
        .iter()
        .filter(|mapping| mapping.exact_type() == exact_type)
        .map(|mapping| mapping.runtime_type())
        .collect::<Vec<_>>();
    let runtime_type = match runtime_types.as_slice() {
        [runtime_type] => *runtime_type,
        _ => {
            return Err(StrongTypeRegistrationPlanBuildError::RuntimeTypeSet {
                exact_type,
                actual: runtime_types,
            });
        }
    };

    let layout = semantic.instance_layout();
    let layout_record = foundation
        .layouts()
        .iter()
        .find(|record| record.id() == layout)
        .ok_or(StrongTypeRegistrationPlanBuildError::MissingLayout(layout))?;
    if layout_record.key().exact_type() != exact_type
        || layout_record.key().target_profile() != &target.wire_id()
        || layout_record.key().representation() != RepresentationRole::ManagedObject
    {
        return Err(
            StrongTypeRegistrationPlanBuildError::InstanceLayoutMismatch { exact_type, layout },
        );
    }
    let scan = semantic.instance_scan();
    let scan_record = foundation
        .scans()
        .iter()
        .find(|record| record.id() == scan)
        .ok_or(StrongTypeRegistrationPlanBuildError::MissingScan(scan))?;
    if scan_record.key().layout() != layout
        || scan_record.key().role() != scoop_identity::ScanRole::ManagedObject
    {
        return Err(StrongTypeRegistrationPlanBuildError::InstanceScanMismatch {
            exact_type,
            scan,
        });
    }
    let vtable = semantic.vtable().table();
    let expected_vtable = scoop_identity::DispatchTableKey::vtable(exact_type);
    if !foundation
        .dispatch_tables()
        .iter()
        .any(|record| record.id() == vtable && record.key() == &expected_vtable)
    {
        return Err(StrongTypeRegistrationPlanBuildError::VtableMismatch {
            exact_type,
            table: vtable,
        });
    }
    for itable in semantic.itables() {
        let expected_itable =
            scoop_identity::DispatchTableKey::itable(exact_type, itable.interface().exact_type());
        if !foundation
            .dispatch_tables()
            .iter()
            .any(|record| record.id() == itable.table() && record.key() == &expected_itable)
        {
            return Err(StrongTypeRegistrationPlanBuildError::ItableMismatch {
                exact_type,
                table: itable.table(),
            });
        }
    }
    let layout_definition = require_definition(
        foundation,
        StrongDefinitionEntity::layout(layout),
        StrongDefinitionRole::Layout,
    )?;
    let layout_primary_atom = require_primary_atom(foundation, layout_definition.id())?;
    let layout_symbol = require_symbol(foundation, PersistentSymbolKey::Layout(layout))?;

    let registration_object =
        require_digest_node(digests, DigestNodeKey::object_definition(primary_atom))?;
    if !registration_object.direct_inputs().is_empty() {
        return Err(
            StrongTypeRegistrationPlanBuildError::RegistrationObjectInputs {
                node: registration_object.id(),
                actual: registration_object.direct_inputs().to_vec(),
            },
        );
    }
    if !registration_object.patch_intents().is_empty() {
        return Err(
            StrongTypeRegistrationPlanBuildError::RegistrationObjectPatches {
                node: registration_object.id(),
                actual: registration_object
                    .patch_intents()
                    .iter()
                    .map(|patch| *patch.key())
                    .collect(),
            },
        );
    }
    let descriptor_definition_node = require_digest_node(
        digests,
        DigestNodeKey::object_definition(descriptor_primary_atom),
    )?;
    let layout_fingerprint_node = require_digest_node(digests, DigestNodeKey::layout(layout))?;
    let registration_fingerprint = require_digest_node(
        digests,
        DigestNodeKey::strong_registration(registration_definition.id()),
    )?;
    if registration_fingerprint.id() != identity.fingerprint_node() {
        return Err(
            StrongTypeRegistrationPlanBuildError::RegistrationDigestMismatch {
                exact_type,
                expected: registration_fingerprint.id(),
                actual: identity.fingerprint_node(),
            },
        );
    }

    let mut expected_inputs = vec![
        DigestInputRefV1::from_node(registration_object),
        DigestInputRefV1::from_node(descriptor_definition_node),
        DigestInputRefV1::from_node(layout_fingerprint_node),
    ];
    expected_inputs.sort_unstable();
    if registration_fingerprint.direct_inputs() != expected_inputs {
        return Err(StrongTypeRegistrationPlanBuildError::DirectInputs {
            node: registration_fingerprint.id(),
            expected: expected_inputs,
            actual: registration_fingerprint.direct_inputs().to_vec(),
        });
    }

    let registration_definition_patch = require_only_patch(
        registration_fingerprint,
        DigestPatchIntentKey::new(
            registration_fingerprint.id(),
            registration_definition.id(),
            DefinitionAtomRole::Primary,
            DigestSemanticFieldRole::RegistrationDefinition,
        ),
    )?;
    let descriptor_definition_patch = require_patch(
        descriptor_definition_node,
        DigestPatchIntentKey::new(
            descriptor_definition_node.id(),
            registration_definition.id(),
            DefinitionAtomRole::Primary,
            DigestSemanticFieldRole::DescriptorDefinition,
        ),
    )?;
    let layout_fingerprint_patch = require_patch(
        layout_fingerprint_node,
        DigestPatchIntentKey::new(
            layout_fingerprint_node.id(),
            registration_definition.id(),
            DefinitionAtomRole::Primary,
            DigestSemanticFieldRole::Layout,
        ),
    )?;

    Ok(StrongTypeRegistrationPlanV1 {
        semantic: semantic.clone(),
        runtime_type,
        symbol,
        definition_plan: registration_definition.id(),
        primary_atom,
        descriptor_symbol,
        descriptor_definition_plan: descriptor_definition.id(),
        descriptor_primary_atom,
        diagnostic_atom,
        layout_symbol,
        layout_definition_plan: layout_definition.id(),
        layout_primary_atom,
        registration_object_node: registration_object.id(),
        descriptor_definition_node: descriptor_definition_node.id(),
        layout_fingerprint_node: layout_fingerprint_node.id(),
        registration_fingerprint_node: registration_fingerprint.id(),
        registration_definition_patch,
        descriptor_definition_patch,
        layout_fingerprint_patch,
    })
}

fn require_descriptor_diagnostic_atom(
    foundation: &OdrFreeLirFoundation,
    plan: ObjectDefinitionPlanId,
    exact_type: PersistentExactTypeId,
) -> Result<ObjectDefinitionAtomId, StrongTypeRegistrationPlanBuildError> {
    let expected = ObjectDefinitionAtomKey::new(
        plan,
        DefinitionAtomRole::AddressTakenConstant,
        DefinitionAtomSubkey::ExactType(exact_type),
    );
    let associated = foundation
        .definition_atoms()
        .iter()
        .filter(|atom| {
            atom.key().plan() == plan && atom.key().role() != DefinitionAtomRole::Primary
        })
        .collect::<Vec<_>>();
    match associated.as_slice() {
        [atom] if atom.key() == &expected => Ok(atom.id()),
        _ => Err(
            StrongTypeRegistrationPlanBuildError::DescriptorAssociatedAtomSet {
                exact_type,
                expected: Box::new(expected),
                actual: associated.iter().map(|atom| atom.id()).collect(),
            },
        ),
    }
}

fn require_definition(
    foundation: &OdrFreeLirFoundation,
    entity: StrongDefinitionEntity,
    role: StrongDefinitionRole,
) -> Result<
    &scoop_identity::CborIdentityRecord<ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    StrongTypeRegistrationPlanBuildError,
> {
    let key = ObjectDefinitionPlanKey::strong(foundation.producer(), entity, role)
        .map_err(StrongTypeRegistrationPlanBuildError::DefinitionIdentity)?;
    foundation
        .definition_plans()
        .iter()
        .find(|record| record.key() == &key)
        .ok_or(StrongTypeRegistrationPlanBuildError::MissingDefinition(
            Box::new(key),
        ))
}

fn require_primary_atom(
    foundation: &OdrFreeLirFoundation,
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

fn require_symbol(
    foundation: &OdrFreeLirFoundation,
    key: PersistentSymbolKey,
) -> Result<PersistentSymbolRequest, StrongTypeRegistrationPlanBuildError> {
    let request = PersistentSymbolRequest::new(key, LinkageClass::ConeStrong)
        .map_err(StrongTypeRegistrationPlanBuildError::Symbol)?;
    foundation
        .contains_symbol_request(request)
        .then_some(request)
        .ok_or(StrongTypeRegistrationPlanBuildError::MissingSymbol(request))
}

fn require_digest_node(
    digests: &StrongDigestFinalizationPlanV1,
    key: DigestNodeKey,
) -> Result<&DigestNodeV1, StrongTypeRegistrationPlanBuildError> {
    digests
        .nodes()
        .iter()
        .find(|node| node.key() == &key)
        .ok_or(StrongTypeRegistrationPlanBuildError::MissingDigestNode(key))
}

fn require_only_patch(
    node: &DigestNodeV1,
    expected: DigestPatchIntentKey,
) -> Result<DigestPatchIntentId, StrongTypeRegistrationPlanBuildError> {
    match node.patch_intents() {
        [patch] if patch.key() == &expected => Ok(patch.id()),
        actual => Err(StrongTypeRegistrationPlanBuildError::PatchSet {
            node: node.id(),
            expected: Box::new(expected),
            actual: actual.iter().map(|patch| *patch.key()).collect(),
        }),
    }
}

fn require_patch(
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongTypeRegistrationPlanBuildError {
    DefinitionIdentity(ObjectDefinitionIdentityError),
    Symbol(PersistentSymbolError),
    DuplicateDescriptor(PersistentExactTypeId),
    ProducerMismatch {
        foundation: ConeIdentity,
        semantics: ConeIdentity,
    },
    TargetMismatch {
        expected: TargetProfileWireId,
        actual: TargetProfileWireId,
    },
    DescriptorSemanticSet {
        expected: Vec<PersistentExactTypeId>,
        actual: Vec<PersistentExactTypeId>,
    },
    TypeSet {
        expected: Vec<PersistentExactTypeId>,
        actual: Vec<PersistentExactTypeId>,
    },
    MissingDefinition(Box<ObjectDefinitionPlanKey>),
    RegistrationDefinitionMismatch {
        exact_type: PersistentExactTypeId,
        expected: ObjectDefinitionPlanId,
        actual: ObjectDefinitionPlanId,
    },
    PrimaryAtomSet {
        plan: ObjectDefinitionPlanId,
        actual: Vec<ObjectDefinitionAtomId>,
    },
    DescriptorAssociatedAtomSet {
        exact_type: PersistentExactTypeId,
        expected: Box<ObjectDefinitionAtomKey>,
        actual: Vec<ObjectDefinitionAtomId>,
    },
    MissingSymbol(PersistentSymbolRequest),
    RuntimeTypeSet {
        exact_type: PersistentExactTypeId,
        actual: Vec<RuntimeTypeId>,
    },
    MissingLayout(PersistentLayoutId),
    MissingScan(scoop_identity::PersistentScanId),
    InstanceLayoutMismatch {
        exact_type: PersistentExactTypeId,
        layout: PersistentLayoutId,
    },
    InstanceScanMismatch {
        exact_type: PersistentExactTypeId,
        scan: scoop_identity::PersistentScanId,
    },
    VtableMismatch {
        exact_type: PersistentExactTypeId,
        table: scoop_identity::PersistentDispatchTableId,
    },
    ItableMismatch {
        exact_type: PersistentExactTypeId,
        table: scoop_identity::PersistentDispatchTableId,
    },
    MissingDigestNode(DigestNodeKey),
    RegistrationObjectInputs {
        node: DigestNodeId,
        actual: Vec<DigestInputRefV1>,
    },
    RegistrationObjectPatches {
        node: DigestNodeId,
        actual: Vec<DigestPatchIntentKey>,
    },
    RegistrationDigestMismatch {
        exact_type: PersistentExactTypeId,
        expected: DigestNodeId,
        actual: DigestNodeId,
    },
    DirectInputs {
        node: DigestNodeId,
        expected: Vec<DigestInputRefV1>,
        actual: Vec<DigestInputRefV1>,
    },
    PatchSet {
        node: DigestNodeId,
        expected: Box<DigestPatchIntentKey>,
        actual: Vec<DigestPatchIntentKey>,
    },
    MissingPatch {
        node: DigestNodeId,
        expected: Box<DigestPatchIntentKey>,
    },
}

impl fmt::Display for StrongTypeRegistrationPlanBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid strong type registration plan: {self:?}")
    }
}

impl std::error::Error for StrongTypeRegistrationPlanBuildError {}

#[cfg(test)]
mod tests;
