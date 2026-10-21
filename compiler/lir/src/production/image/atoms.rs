//! Typed object atoms owned by the Cone image descriptor.

use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConeImageSupportAtomsV1 {
    coordinate_group: ObjectDefinitionAtomId,
    coordinate_name: ObjectDefinitionAtomId,
    coordinate_version: ObjectDefinitionAtomId,
    dependencies: ObjectDefinitionAtomId,
    static_storages: ObjectDefinitionAtomId,
    immortal_objects: ObjectDefinitionAtomId,
    initialization_units: ObjectDefinitionAtomId,
    type_registrations: ObjectDefinitionAtomId,
    safepoints: ObjectDefinitionAtomId,
    callables: ObjectDefinitionAtomId,
    array_bounds_message: ObjectDefinitionAtomId,
    array_size_overflow_message: ObjectDefinitionAtomId,
}

impl ConeImageSupportAtomsV1 {
    pub const fn coordinate_group(self) -> ObjectDefinitionAtomId {
        self.coordinate_group
    }

    pub const fn coordinate_name(self) -> ObjectDefinitionAtomId {
        self.coordinate_name
    }

    pub const fn coordinate_version(self) -> ObjectDefinitionAtomId {
        self.coordinate_version
    }

    pub const fn dependencies(self) -> ObjectDefinitionAtomId {
        self.dependencies
    }

    pub const fn static_storages(self) -> ObjectDefinitionAtomId {
        self.static_storages
    }

    pub const fn immortal_objects(self) -> ObjectDefinitionAtomId {
        self.immortal_objects
    }

    pub const fn initialization_units(self) -> ObjectDefinitionAtomId {
        self.initialization_units
    }

    pub const fn type_registrations(self) -> ObjectDefinitionAtomId {
        self.type_registrations
    }

    pub const fn safepoints(self) -> ObjectDefinitionAtomId {
        self.safepoints
    }

    pub const fn callables(self) -> ObjectDefinitionAtomId {
        self.callables
    }

    pub const fn array_bounds_message(self) -> ObjectDefinitionAtomId {
        self.array_bounds_message
    }

    pub const fn array_size_overflow_message(self) -> ObjectDefinitionAtomId {
        self.array_size_overflow_message
    }
}

impl WireEncode for ConeImageSupportAtomsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_image_support_atoms(
            encoder,
            &self.array_bounds_message,
            &self.array_size_overflow_message,
        )
    }
}

#[derive(Debug)]
pub(super) struct DecodedConeImageSupportAtomsV1 {
    array_bounds_message: DecodedPersistentId<ObjectDefinitionAtomId>,
    array_size_overflow_message: DecodedPersistentId<ObjectDefinitionAtomId>,
}

impl DecodedConeImageSupportAtomsV1 {
    pub(super) fn link_trap_atoms(
        &self,
        identities: &mut scoop_identity::ValidatedIdentityGraph,
    ) -> Result<[ObjectDefinitionAtomId; 2], crate::LinkDataError> {
        use crate::link_data::link_error;
        use scoop_identity::PersistentIdResolver;
        Ok([
            identities
                .resolve(self.array_bounds_message)
                .map_err(link_error)?,
            identities
                .resolve(self.array_size_overflow_message)
                .map_err(link_error)?,
        ])
    }
}

impl WireEncode for DecodedConeImageSupportAtomsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_image_support_atoms(
            encoder,
            &self.array_bounds_message,
            &self.array_size_overflow_message,
        )
    }
}

impl WireDecode for DecodedConeImageSupportAtomsV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            array_bounds_message: decoder.field(11, DecodedPersistentId::decode)?,
            array_size_overflow_message: decoder.field(12, DecodedPersistentId::decode)?,
        })
    }
}

pub(super) fn require_image_atoms(
    foundation: &ConeLirFoundation,
    definition: ObjectDefinitionPlanId,
) -> Result<(ObjectDefinitionAtomId, ConeImageSupportAtomsV1), ConeImagePlanBuildError> {
    let primary_key = ObjectDefinitionAtomKey::new(
        definition,
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    );
    let support_keys = [
        image_support_key(
            definition,
            DefinitionAtomRole::AddressTakenConstant,
            ConeImageSupportRole::CoordinateGroup,
        ),
        image_support_key(
            definition,
            DefinitionAtomRole::AddressTakenConstant,
            ConeImageSupportRole::CoordinateName,
        ),
        image_support_key(
            definition,
            DefinitionAtomRole::AddressTakenConstant,
            ConeImageSupportRole::CoordinateVersion,
        ),
        image_support_key(
            definition,
            DefinitionAtomRole::RuntimeRecord,
            ConeImageSupportRole::Dependencies,
        ),
        image_support_key(
            definition,
            DefinitionAtomRole::RuntimeRecord,
            ConeImageSupportRole::StaticStorages,
        ),
        image_support_key(
            definition,
            DefinitionAtomRole::RuntimeRecord,
            ConeImageSupportRole::ImmortalObjects,
        ),
        image_support_key(
            definition,
            DefinitionAtomRole::RuntimeRecord,
            ConeImageSupportRole::InitializationUnits,
        ),
        image_support_key(
            definition,
            DefinitionAtomRole::RuntimeRecord,
            ConeImageSupportRole::TypeRegistrations,
        ),
        image_support_key(
            definition,
            DefinitionAtomRole::RuntimeRecord,
            ConeImageSupportRole::Safepoints,
        ),
        image_support_key(
            definition,
            DefinitionAtomRole::RuntimeRecord,
            ConeImageSupportRole::Callables,
        ),
        image_support_key(
            definition,
            DefinitionAtomRole::AddressTakenConstant,
            ConeImageSupportRole::ArrayBoundsMessage,
        ),
        image_support_key(
            definition,
            DefinitionAtomRole::AddressTakenConstant,
            ConeImageSupportRole::ArraySizeOverflowMessage,
        ),
    ];
    let mut expected = Vec::with_capacity(13);
    expected.push(primary_key.clone());
    expected.extend(support_keys.iter().cloned());
    expected.sort_unstable();
    let mut actual = foundation
        .definition_atoms_for_plan(definition)
        .map(|record| record.key().clone())
        .collect::<Vec<_>>();
    actual.sort_unstable();
    if actual != expected {
        return Err(ConeImagePlanBuildError::AtomSet { expected, actual });
    }

    let atom_id = |key: &ObjectDefinitionAtomKey| {
        foundation
            .definition_atoms_for_plan(key.plan())
            .find(|record| record.key() == key)
            .expect("the exact image atom set was validated")
            .id()
    };
    Ok((
        atom_id(&primary_key),
        ConeImageSupportAtomsV1 {
            coordinate_group: atom_id(&support_keys[0]),
            coordinate_name: atom_id(&support_keys[1]),
            coordinate_version: atom_id(&support_keys[2]),
            dependencies: atom_id(&support_keys[3]),
            static_storages: atom_id(&support_keys[4]),
            immortal_objects: atom_id(&support_keys[5]),
            initialization_units: atom_id(&support_keys[6]),
            type_registrations: atom_id(&support_keys[7]),
            safepoints: atom_id(&support_keys[8]),
            callables: atom_id(&support_keys[9]),
            array_bounds_message: atom_id(&support_keys[10]),
            array_size_overflow_message: atom_id(&support_keys[11]),
        },
    ))
}

fn image_support_key(
    definition: ObjectDefinitionPlanId,
    role: DefinitionAtomRole,
    support: ConeImageSupportRole,
) -> ObjectDefinitionAtomKey {
    ObjectDefinitionAtomKey::new(
        definition,
        role,
        DefinitionAtomSubkey::ConeImageSupport(support),
    )
}

fn encode_image_support_atoms(
    encoder: &mut Encoder,
    array_bounds_message: &impl WireEncode,
    array_size_overflow_message: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encoder.field(11)?;
    array_bounds_message.encode(encoder)?;
    encoder.field(12)?;
    array_size_overflow_message.encode(encoder)
}
