//! Semantic plans for immutable String and byte-array objects and their type registrations.

use crate::{GlobalInit, LirTargetProfile, Module, PointerKind, RefScan, TypeDescriptorRef};
use scoop_identity::{
    ConeIdentity, LinkageClass, PersistentExactTypeId, PersistentImmortalObjectId,
    PersistentSymbolRequest,
};
use std::collections::BTreeMap;
use std::fmt;

/// Typed origin of the type registration referenced by an immortal object.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImmortalObjectTypeRegistrationRefV1 {
    Local(PersistentExactTypeId),
    DependencyExternal {
        provider: ConeIdentity,
        exact: PersistentExactTypeId,
    },
}

impl ImmortalObjectTypeRegistrationRefV1 {
    pub const fn exact_type(self) -> PersistentExactTypeId {
        match self {
            Self::Local(exact_type)
            | Self::DependencyExternal {
                exact: exact_type, ..
            } => exact_type,
        }
    }
}

/// Member-independent semantics of one read-only immortal object.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StrongImmortalObjectSemanticPlanV1 {
    object: PersistentImmortalObjectId,
    symbol: PersistentSymbolRequest,
    object_size: u64,
    required_alignment: u64,
    type_registration: ImmortalObjectTypeRegistrationRefV1,
}

impl StrongImmortalObjectSemanticPlanV1 {
    pub const fn object(self) -> PersistentImmortalObjectId {
        self.object
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
        Self::from_globals(module.cone, &module.meta, &module.globals, string_type)
    }

    pub(crate) const fn from_artifact(
        producer: ConeIdentity,
        objects: Vec<StrongImmortalObjectSemanticPlanV1>,
    ) -> Self {
        Self { producer, objects }
    }

    pub(super) fn from_globals(
        producer: ConeIdentity,
        meta: &crate::LirMeta,
        globals: &la_arena::Arena<crate::Global>,
        string_type: ImmortalObjectTypeRegistrationRefV1,
    ) -> Result<Self, StrongImmortalObjectSemanticPlanBuildError> {
        let mut objects = BTreeMap::new();
        for (_, global) in globals.iter() {
            let (identity, object_size, required_alignment, type_registration) =
                match &global.init {
                    GlobalInit::StringConst { identity, value } => (
                        identity,
                        string_object_size(meta.target_profile, value.len())?,
                        string_object_alignment(meta.target_profile),
                        string_type,
                    ),
                    GlobalInit::ByteArrayConst {
                        identity,
                        array_type,
                        bytes,
                    } => {
                        let array = &meta.arrays[*array_type];
                        if array.kind != crate::ArrayKind::Immutable
                            || array.element != crate::LirType::I8
                        {
                            return Err(StrongImmortalObjectSemanticPlanBuildError::ByteArrayType(
                                *array_type,
                            ));
                        }
                        let count = u64::try_from(bytes.len()).map_err(|_| {
                            StrongImmortalObjectSemanticPlanBuildError::ObjectSizeOverflow
                        })?;
                        let size = array.layout.allocation_size(count).ok_or(
                            StrongImmortalObjectSemanticPlanBuildError::ObjectSizeOverflow,
                        )?;
                        (
                            identity,
                            size,
                            array.layout.instance().instance_alignment(),
                            type_registration(meta, array.type_descriptor)?,
                        )
                    }
                    _ => continue,
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
            if !matches!(
                symbol.linkage(),
                LinkageClass::ConeStrong | LinkageClass::OdrWeak
            ) {
                return Err(StrongImmortalObjectSemanticPlanBuildError::Linkage {
                    object: identity.identity_record().id(),
                    actual: symbol.linkage(),
                });
            }
            let plan = StrongImmortalObjectSemanticPlanV1 {
                object: identity.identity_record().id(),
                symbol,
                object_size,
                required_alignment,
                type_registration,
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
    type_registration(&module.meta, module.meta.well_known_type_descriptors.string)
}

fn type_registration(
    meta: &crate::LirMeta,
    descriptor: TypeDescriptorRef,
) -> Result<ImmortalObjectTypeRegistrationRefV1, StrongImmortalObjectSemanticPlanBuildError> {
    match descriptor {
        TypeDescriptorRef::Local(id) => {
            if id.into_raw().into_u32() as usize >= meta.type_descriptors.len() {
                return Err(StrongImmortalObjectSemanticPlanBuildError::MissingTypeDescriptor);
            }
            Ok(ImmortalObjectTypeRegistrationRefV1::Local(
                meta.type_descriptors[id].identity.exact_type(),
            ))
        }
        TypeDescriptorRef::External(id) => {
            if id.into_raw().into_u32() as usize >= meta.external_type_descriptors.len() {
                return Err(StrongImmortalObjectSemanticPlanBuildError::MissingTypeDescriptor);
            }
            let descriptor = meta.external_type_descriptors[id];
            Ok(ImmortalObjectTypeRegistrationRefV1::DependencyExternal {
                provider: descriptor.provider(),
                exact: descriptor.target(),
            })
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
    MissingTypeDescriptor,
    ByteArrayType(crate::ArrayTypeId),

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
