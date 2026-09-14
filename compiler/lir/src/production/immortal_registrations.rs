//! Complete semantic and writer-side production plans for strong immortal-object registrations.

use std::collections::BTreeMap;
use std::fmt;

pub use scoop_identity::PersistentImmortalObjectId;
use scoop_identity::{
    ConeIdentity, DefinitionAtomRole, DigestNodeId, DigestNodeKey, DigestPatchIntentId,
    DigestPatchIntentKey, DigestSemanticFieldRole, LinkageClass, ObjectDefinitionAtomId,
    ObjectDefinitionIdentityError, ObjectDefinitionPlanId, ObjectDefinitionPlanKey,
    PersistentExactTypeId, PersistentSymbolError, PersistentSymbolKey, PersistentSymbolRequest,
    StrongDefinitionEntity, StrongDefinitionRole,
};

use crate::{
    DigestInputRefV1, DigestNodeV1, GlobalId, GlobalInit, LirTargetProfile, Module,
    OdrFreeLirFoundation, PointerKind, RefScan, StrongDigestFinalizationPlanV1,
    StrongRegistrationIdentitySurfaceV1, TypeDescriptorRef,
};

/// Typed origin of the type registration referenced by an immortal object.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImmortalObjectTypeRegistrationRefV1 {
    Local(PersistentExactTypeId),
    CoreExternal(PersistentExactTypeId),
}

impl ImmortalObjectTypeRegistrationRefV1 {
    pub const fn exact_type(self) -> PersistentExactTypeId {
        match self {
            Self::Local(exact_type) | Self::CoreExternal(exact_type) => exact_type,
        }
    }
}

/// Member-independent semantics of one read-only immortal object.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StrongImmortalObjectSemanticPlanV1 {
    object: PersistentImmortalObjectId,
    global: GlobalId,
    symbol: PersistentSymbolRequest,
    object_size: u64,
    required_alignment: u64,
    type_registration: ImmortalObjectTypeRegistrationRefV1,
}

impl StrongImmortalObjectSemanticPlanV1 {
    pub const fn object(self) -> PersistentImmortalObjectId {
        self.object
    }

    pub const fn global(self) -> GlobalId {
        self.global
    }

    pub const fn symbol(self) -> PersistentSymbolRequest {
        self.symbol
    }

    pub const fn object_size(self) -> u64 {
        self.object_size
    }

    pub const fn required_alignment(self) -> u64 {
        self.required_alignment
    }

    pub const fn type_registration(self) -> PersistentExactTypeId {
        self.type_registration.exact_type()
    }

    pub const fn type_registration_ref(self) -> ImmortalObjectTypeRegistrationRefV1 {
        self.type_registration
    }
}

/// Proof that every final LIR immortal object has one canonical semantic plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongImmortalObjectSemanticPlanSetV1 {
    producer: ConeIdentity,
    objects: Vec<StrongImmortalObjectSemanticPlanV1>,
}

impl StrongImmortalObjectSemanticPlanSetV1 {
    pub fn from_module(
        module: &Module,
    ) -> Result<Self, StrongImmortalObjectSemanticPlanBuildError> {
        let string_type = string_type_registration(module)?;
        Self::from_globals(
            module.cone,
            module.meta.target_profile,
            &module.globals,
            string_type,
        )
    }

    fn from_globals(
        producer: ConeIdentity,
        target: LirTargetProfile,
        globals: &la_arena::Arena<crate::Global>,
        string_type: ImmortalObjectTypeRegistrationRefV1,
    ) -> Result<Self, StrongImmortalObjectSemanticPlanBuildError> {
        let mut objects = BTreeMap::new();
        for (global_id, global) in globals.iter() {
            let GlobalInit::StringConst { identity, value } = &global.init else {
                continue;
            };
            if global.address_kind != PointerKind::Managed {
                return Err(StrongImmortalObjectSemanticPlanBuildError::AddressKind {
                    object: identity.identity_record().id(),
                    actual: global.address_kind,
                });
            }
            if global.scan != RefScan::None {
                return Err(StrongImmortalObjectSemanticPlanBuildError::Scan {
                    object: identity.identity_record().id(),
                    actual: global.scan.clone(),
                });
            }
            let symbol = identity.symbol_request();
            if symbol.linkage() != LinkageClass::ConeStrong {
                return Err(StrongImmortalObjectSemanticPlanBuildError::Linkage {
                    object: identity.identity_record().id(),
                    actual: symbol.linkage(),
                });
            }
            let object_size = string_object_size(target, value.len())?;
            let plan = StrongImmortalObjectSemanticPlanV1 {
                object: identity.identity_record().id(),
                global: global_id,
                symbol,
                object_size,
                required_alignment: string_object_alignment(target),
                type_registration: string_type,
            };
            if objects.insert(plan.object, plan).is_some() {
                return Err(StrongImmortalObjectSemanticPlanBuildError::DuplicateObject(
                    plan.object,
                ));
            }
        }
        Ok(Self {
            producer,
            objects: objects.into_values().collect(),
        })
    }

    pub const fn producer(&self) -> ConeIdentity {
        self.producer
    }

    pub fn objects(&self) -> &[StrongImmortalObjectSemanticPlanV1] {
        &self.objects
    }
}

fn string_type_registration(
    module: &Module,
) -> Result<ImmortalObjectTypeRegistrationRefV1, StrongImmortalObjectSemanticPlanBuildError> {
    match module.meta.well_known_type_descriptors.string {
        TypeDescriptorRef::Local(id) => {
            if id.into_raw().into_u32() as usize >= module.meta.type_descriptors.len() {
                return Err(
                    StrongImmortalObjectSemanticPlanBuildError::MissingStringTypeDescriptor,
                );
            }
            Ok(ImmortalObjectTypeRegistrationRefV1::Local(
                module.meta.type_descriptors[id].identity.exact_type(),
            ))
        }
        TypeDescriptorRef::CoreExternal(id) => {
            if id.into_raw().into_u32() as usize >= module.meta.core_external_type_descriptors.len()
            {
                return Err(
                    StrongImmortalObjectSemanticPlanBuildError::MissingStringTypeDescriptor,
                );
            }
            Ok(ImmortalObjectTypeRegistrationRefV1::CoreExternal(
                module.meta.core_external_type_descriptors[id].target(),
            ))
        }
    }
}

fn string_object_alignment(target: LirTargetProfile) -> u64 {
    target.metadata_pointer_layout().alignment_bytes().max(
        target
            .scalar_layout(crate::BackendScalarKind::I64)
            .alignment_bytes(),
    )
}

fn string_object_size(
    target: LirTargetProfile,
    byte_length: usize,
) -> Result<u64, StrongImmortalObjectSemanticPlanBuildError> {
    let pointer = target.metadata_pointer_layout().size_bytes();
    let word = target
        .scalar_layout(crate::BackendScalarKind::I64)
        .size_bytes();
    let byte_length = u64::try_from(byte_length)
        .map_err(|_| StrongImmortalObjectSemanticPlanBuildError::ObjectSizeOverflow)?;
    let unaligned = pointer
        .checked_add(word)
        .and_then(|size| size.checked_add(word))
        .and_then(|size| size.checked_add(byte_length))
        .ok_or(StrongImmortalObjectSemanticPlanBuildError::ObjectSizeOverflow)?;
    let alignment = string_object_alignment(target);
    let size = unaligned
        .checked_add(alignment - 1)
        .map(|size| size & !(alignment - 1))
        .ok_or(StrongImmortalObjectSemanticPlanBuildError::ObjectSizeOverflow)?;
    if size > target.contract().maximum_managed_object_size() {
        return Err(StrongImmortalObjectSemanticPlanBuildError::ObjectTooLarge {
            size,
            maximum: target.contract().maximum_managed_object_size(),
        });
    }
    Ok(size)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongImmortalObjectSemanticPlanBuildError {
    MissingStringTypeDescriptor,
    DuplicateObject(PersistentImmortalObjectId),
    AddressKind {
        object: PersistentImmortalObjectId,
        actual: PointerKind,
    },
    Scan {
        object: PersistentImmortalObjectId,
        actual: RefScan,
    },
    Linkage {
        object: PersistentImmortalObjectId,
        actual: LinkageClass,
    },
    ObjectSizeOverflow,
    ObjectTooLarge {
        size: u64,
        maximum: u64,
    },
}

impl fmt::Display for StrongImmortalObjectSemanticPlanBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid strong immortal-object semantic plan: {self:?}"
        )
    }
}

impl std::error::Error for StrongImmortalObjectSemanticPlanBuildError {}

/// All identities and graph relations needed to emit one immortal-object registration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StrongImmortalObjectRegistrationPlanV1 {
    semantic: StrongImmortalObjectSemanticPlanV1,
    registration_symbol: PersistentSymbolRequest,
    registration_definition_plan: ObjectDefinitionPlanId,
    registration_primary_atom: ObjectDefinitionAtomId,
    object_definition_plan: ObjectDefinitionPlanId,
    object_primary_atom: ObjectDefinitionAtomId,
    type_registration_symbol: PersistentSymbolRequest,
    registration_object_node: DigestNodeId,
    object_definition_node: DigestNodeId,
    registration_fingerprint_node: DigestNodeId,
    registration_definition_patch: DigestPatchIntentId,
}

impl StrongImmortalObjectRegistrationPlanV1 {
    pub const fn semantic(self) -> StrongImmortalObjectSemanticPlanV1 {
        self.semantic
    }

    pub const fn object(self) -> PersistentImmortalObjectId {
        self.semantic.object()
    }

    pub const fn global(self) -> GlobalId {
        self.semantic.global()
    }

    pub const fn object_symbol(self) -> PersistentSymbolRequest {
        self.semantic.symbol()
    }

    pub const fn object_size(self) -> u64 {
        self.semantic.object_size()
    }

    pub const fn required_alignment(self) -> u64 {
        self.semantic.required_alignment()
    }

    pub const fn type_registration(self) -> PersistentExactTypeId {
        self.semantic.type_registration()
    }

    pub const fn registration_symbol(self) -> PersistentSymbolRequest {
        self.registration_symbol
    }

    pub const fn registration_definition_plan(self) -> ObjectDefinitionPlanId {
        self.registration_definition_plan
    }

    pub const fn registration_primary_atom(self) -> ObjectDefinitionAtomId {
        self.registration_primary_atom
    }

    pub const fn object_definition_plan(self) -> ObjectDefinitionPlanId {
        self.object_definition_plan
    }

    pub const fn object_primary_atom(self) -> ObjectDefinitionAtomId {
        self.object_primary_atom
    }

    pub const fn type_registration_symbol(self) -> PersistentSymbolRequest {
        self.type_registration_symbol
    }

    pub const fn registration_object_node(self) -> DigestNodeId {
        self.registration_object_node
    }

    pub const fn object_definition_node(self) -> DigestNodeId {
        self.object_definition_node
    }

    pub const fn registration_fingerprint_node(self) -> DigestNodeId {
        self.registration_fingerprint_node
    }

    pub const fn registration_definition_patch(self) -> DigestPatchIntentId {
        self.registration_definition_patch
    }
}

/// Proof that every final LIR immortal object has one complete strong registration plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongImmortalObjectRegistrationPlanSetV1 {
    producer: ConeIdentity,
    registrations: Vec<StrongImmortalObjectRegistrationPlanV1>,
}

impl StrongImmortalObjectRegistrationPlanSetV1 {
    pub fn new(
        foundation: &OdrFreeLirFoundation,
        identities: &StrongRegistrationIdentitySurfaceV1,
        semantics: &StrongImmortalObjectSemanticPlanSetV1,
        digests: &StrongDigestFinalizationPlanV1,
    ) -> Result<Self, StrongImmortalObjectRegistrationPlanBuildError> {
        if semantics.producer() != foundation.producer() {
            return Err(
                StrongImmortalObjectRegistrationPlanBuildError::ProducerMismatch {
                    foundation: foundation.producer(),
                    semantics: semantics.producer(),
                },
            );
        }
        let expected = semantics
            .objects()
            .iter()
            .map(|object| object.object())
            .collect::<Vec<_>>();
        let actual = identities
            .immortal_objects()
            .iter()
            .map(|registration| registration.semantic_id())
            .collect::<Vec<_>>();
        if expected != actual {
            return Err(StrongImmortalObjectRegistrationPlanBuildError::ObjectSet {
                expected,
                actual,
            });
        }
        for semantic in semantics.objects() {
            let local_type_registration_count = identities
                .type_registrations()
                .iter()
                .filter(|registration| registration.semantic_id() == semantic.type_registration())
                .count();
            match semantic.type_registration_ref() {
                ImmortalObjectTypeRegistrationRefV1::Local(_)
                    if local_type_registration_count != 1 =>
                {
                    return Err(
                        StrongImmortalObjectRegistrationPlanBuildError::LocalTypeRegistrationSet {
                            object: semantic.object(),
                            type_registration: semantic.type_registration(),
                            actual: local_type_registration_count,
                        },
                    );
                }
                ImmortalObjectTypeRegistrationRefV1::CoreExternal(_)
                    if local_type_registration_count != 0 =>
                {
                    return Err(
                        StrongImmortalObjectRegistrationPlanBuildError::CoreExternalTypeRegistrationConflict {
                            object: semantic.object(),
                            type_registration: semantic.type_registration(),
                        },
                    );
                }
                ImmortalObjectTypeRegistrationRefV1::Local(_)
                | ImmortalObjectTypeRegistrationRefV1::CoreExternal(_) => {}
            }
        }
        let registrations = semantics
            .objects()
            .iter()
            .zip(identities.immortal_objects())
            .map(|(semantic, identity)| build_registration(foundation, semantic, identity, digests))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            producer: foundation.producer(),
            registrations,
        })
    }

    pub const fn producer(&self) -> ConeIdentity {
        self.producer
    }

    pub fn registrations(&self) -> &[StrongImmortalObjectRegistrationPlanV1] {
        &self.registrations
    }
}

fn build_registration(
    foundation: &OdrFreeLirFoundation,
    semantic: &StrongImmortalObjectSemanticPlanV1,
    identity: &crate::StrongRegistrationIdentityV1<PersistentImmortalObjectId>,
    digests: &StrongDigestFinalizationPlanV1,
) -> Result<StrongImmortalObjectRegistrationPlanV1, StrongImmortalObjectRegistrationPlanBuildError>
{
    let object = semantic.object();
    if !foundation.contains_immortal_object(object) {
        return Err(StrongImmortalObjectRegistrationPlanBuildError::MissingObject(object));
    }

    let object_definition = require_definition(
        foundation,
        StrongDefinitionEntity::immortal_object(object),
        StrongDefinitionRole::ImmortalObject,
    )?;
    let object_primary_atom = require_primary_atom(foundation, object_definition.id())?;
    require_symbol(foundation, semantic.symbol())?;

    let registration_definition = require_definition(
        foundation,
        StrongDefinitionEntity::immortal_object(object),
        StrongDefinitionRole::ImmortalRegistration,
    )?;
    if registration_definition.id() != identity.definition_plan() {
        return Err(
            StrongImmortalObjectRegistrationPlanBuildError::RegistrationDefinitionMismatch {
                object,
                expected: registration_definition.id(),
                actual: identity.definition_plan(),
            },
        );
    }
    let registration_primary_atom = require_primary_atom(foundation, registration_definition.id())?;
    let registration_symbol = require_symbol_key(
        foundation,
        PersistentSymbolKey::ImmortalRegistration(object),
    )?;
    let type_registration_symbol = PersistentSymbolRequest::new(
        PersistentSymbolKey::TypeRegistration(semantic.type_registration()),
        LinkageClass::ConeStrong,
    )
    .map_err(StrongImmortalObjectRegistrationPlanBuildError::Symbol)?;

    let registration_object = require_digest_node(
        digests,
        DigestNodeKey::object_definition(registration_primary_atom),
    )?;
    if !registration_object.direct_inputs().is_empty() {
        return Err(
            StrongImmortalObjectRegistrationPlanBuildError::RegistrationObjectInputs {
                node: registration_object.id(),
                actual: registration_object.direct_inputs().to_vec(),
            },
        );
    }
    if !registration_object.patch_intents().is_empty() {
        return Err(
            StrongImmortalObjectRegistrationPlanBuildError::RegistrationObjectPatches {
                node: registration_object.id(),
                actual: registration_object
                    .patch_intents()
                    .iter()
                    .map(|patch| *patch.key())
                    .collect(),
            },
        );
    }
    let object_node = require_digest_node(
        digests,
        DigestNodeKey::object_definition(object_primary_atom),
    )?;
    if !object_node.direct_inputs().is_empty() {
        return Err(
            StrongImmortalObjectRegistrationPlanBuildError::ImmortalObjectInputs {
                node: object_node.id(),
                actual: object_node.direct_inputs().to_vec(),
            },
        );
    }
    if !object_node.patch_intents().is_empty() {
        return Err(
            StrongImmortalObjectRegistrationPlanBuildError::ImmortalObjectPatches {
                node: object_node.id(),
                actual: object_node
                    .patch_intents()
                    .iter()
                    .map(|patch| *patch.key())
                    .collect(),
            },
        );
    }
    let registration = require_digest_node(
        digests,
        DigestNodeKey::strong_registration(registration_definition.id()),
    )?;
    if registration.id() != identity.fingerprint_node() {
        return Err(
            StrongImmortalObjectRegistrationPlanBuildError::RegistrationDigestMismatch {
                object,
                expected: registration.id(),
                actual: identity.fingerprint_node(),
            },
        );
    }
    let mut expected_inputs = vec![
        DigestInputRefV1::from_node(registration_object),
        DigestInputRefV1::from_node(object_node),
    ];
    expected_inputs.sort_unstable();
    if registration.direct_inputs() != expected_inputs {
        return Err(
            StrongImmortalObjectRegistrationPlanBuildError::DirectInputs {
                node: registration.id(),
                expected: expected_inputs,
                actual: registration.direct_inputs().to_vec(),
            },
        );
    }
    let registration_definition_patch = require_only_patch(
        registration,
        DigestPatchIntentKey::new(
            registration.id(),
            registration_definition.id(),
            DefinitionAtomRole::Primary,
            DigestSemanticFieldRole::RegistrationDefinition,
        ),
    )?;

    Ok(StrongImmortalObjectRegistrationPlanV1 {
        semantic: *semantic,
        registration_symbol,
        registration_definition_plan: registration_definition.id(),
        registration_primary_atom,
        object_definition_plan: object_definition.id(),
        object_primary_atom,
        type_registration_symbol,
        registration_object_node: registration_object.id(),
        object_definition_node: object_node.id(),
        registration_fingerprint_node: registration.id(),
        registration_definition_patch,
    })
}

fn require_definition(
    foundation: &OdrFreeLirFoundation,
    entity: StrongDefinitionEntity,
    role: StrongDefinitionRole,
) -> Result<&crate::DefinitionPlanRecord, StrongImmortalObjectRegistrationPlanBuildError> {
    let key = ObjectDefinitionPlanKey::strong(foundation.producer(), entity, role)
        .map_err(StrongImmortalObjectRegistrationPlanBuildError::DefinitionIdentity)?;
    foundation
        .definition_plans()
        .iter()
        .find(|record| record.key() == &key)
        .ok_or(StrongImmortalObjectRegistrationPlanBuildError::MissingDefinition(Box::new(key)))
}

fn require_primary_atom(
    foundation: &OdrFreeLirFoundation,
    definition: ObjectDefinitionPlanId,
) -> Result<ObjectDefinitionAtomId, StrongImmortalObjectRegistrationPlanBuildError> {
    let atoms = foundation
        .definition_atoms()
        .iter()
        .filter(|record| {
            record.key().plan() == definition && record.key().role() == DefinitionAtomRole::Primary
        })
        .map(|record| record.id())
        .collect::<Vec<_>>();
    match atoms.as_slice() {
        [atom] => Ok(*atom),
        _ => Err(
            StrongImmortalObjectRegistrationPlanBuildError::PrimaryAtomSet {
                plan: definition,
                actual: atoms,
            },
        ),
    }
}

fn require_symbol_key(
    foundation: &OdrFreeLirFoundation,
    key: PersistentSymbolKey,
) -> Result<PersistentSymbolRequest, StrongImmortalObjectRegistrationPlanBuildError> {
    let symbol = PersistentSymbolRequest::new(key, LinkageClass::ConeStrong)
        .map_err(StrongImmortalObjectRegistrationPlanBuildError::Symbol)?;
    require_symbol(foundation, symbol)?;
    Ok(symbol)
}

fn require_symbol(
    foundation: &OdrFreeLirFoundation,
    symbol: PersistentSymbolRequest,
) -> Result<(), StrongImmortalObjectRegistrationPlanBuildError> {
    if foundation.contains_symbol_request(symbol) {
        Ok(())
    } else {
        Err(StrongImmortalObjectRegistrationPlanBuildError::MissingSymbol(symbol))
    }
}

fn require_digest_node(
    digests: &StrongDigestFinalizationPlanV1,
    key: DigestNodeKey,
) -> Result<&DigestNodeV1, StrongImmortalObjectRegistrationPlanBuildError> {
    digests
        .nodes()
        .iter()
        .find(|node| node.key() == &key)
        .ok_or(StrongImmortalObjectRegistrationPlanBuildError::MissingDigestNode(key))
}

fn require_only_patch(
    node: &DigestNodeV1,
    expected: DigestPatchIntentKey,
) -> Result<DigestPatchIntentId, StrongImmortalObjectRegistrationPlanBuildError> {
    match node.patch_intents() {
        [patch] if patch.key() == &expected => Ok(patch.id()),
        actual => Err(StrongImmortalObjectRegistrationPlanBuildError::PatchSet {
            node: node.id(),
            expected: Box::new(expected),
            actual: actual.iter().map(|patch| *patch.key()).collect(),
        }),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongImmortalObjectRegistrationPlanBuildError {
    DefinitionIdentity(ObjectDefinitionIdentityError),
    Symbol(PersistentSymbolError),
    ProducerMismatch {
        foundation: ConeIdentity,
        semantics: ConeIdentity,
    },
    ObjectSet {
        expected: Vec<PersistentImmortalObjectId>,
        actual: Vec<PersistentImmortalObjectId>,
    },
    MissingObject(PersistentImmortalObjectId),
    LocalTypeRegistrationSet {
        object: PersistentImmortalObjectId,
        type_registration: PersistentExactTypeId,
        actual: usize,
    },
    CoreExternalTypeRegistrationConflict {
        object: PersistentImmortalObjectId,
        type_registration: PersistentExactTypeId,
    },
    MissingDefinition(Box<ObjectDefinitionPlanKey>),
    RegistrationDefinitionMismatch {
        object: PersistentImmortalObjectId,
        expected: ObjectDefinitionPlanId,
        actual: ObjectDefinitionPlanId,
    },
    PrimaryAtomSet {
        plan: ObjectDefinitionPlanId,
        actual: Vec<ObjectDefinitionAtomId>,
    },
    MissingSymbol(PersistentSymbolRequest),
    MissingDigestNode(DigestNodeKey),
    RegistrationDigestMismatch {
        object: PersistentImmortalObjectId,
        expected: DigestNodeId,
        actual: DigestNodeId,
    },
    DirectInputs {
        node: DigestNodeId,
        expected: Vec<DigestInputRefV1>,
        actual: Vec<DigestInputRefV1>,
    },
    RegistrationObjectInputs {
        node: DigestNodeId,
        actual: Vec<DigestInputRefV1>,
    },
    RegistrationObjectPatches {
        node: DigestNodeId,
        actual: Vec<DigestPatchIntentKey>,
    },
    ImmortalObjectInputs {
        node: DigestNodeId,
        actual: Vec<DigestInputRefV1>,
    },
    ImmortalObjectPatches {
        node: DigestNodeId,
        actual: Vec<DigestPatchIntentKey>,
    },
    PatchSet {
        node: DigestNodeId,
        expected: Box<DigestPatchIntentKey>,
        actual: Vec<DigestPatchIntentKey>,
    },
}

impl fmt::Display for StrongImmortalObjectRegistrationPlanBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid strong immortal-object registration plan: {self:?}"
        )
    }
}

impl std::error::Error for StrongImmortalObjectRegistrationPlanBuildError {}

#[cfg(test)]
mod tests;
