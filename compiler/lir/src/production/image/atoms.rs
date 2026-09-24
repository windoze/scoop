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
            &self.coordinate_group,
            &self.coordinate_name,
            &self.coordinate_version,
            &self.dependencies,
            &self.static_storages,
            &self.immortal_objects,
            &self.initialization_units,
            &self.type_registrations,
            &self.safepoints,
            &self.callables,
            &self.array_bounds_message,
            &self.array_size_overflow_message,
        )
    }
}

#[derive(Debug)]
pub(super) struct DecodedConeImageSupportAtomsV1 {
    coordinate_group: DecodedPersistentId<ObjectDefinitionAtomId>,
    coordinate_name: DecodedPersistentId<ObjectDefinitionAtomId>,
    coordinate_version: DecodedPersistentId<ObjectDefinitionAtomId>,
    dependencies: DecodedPersistentId<ObjectDefinitionAtomId>,
    static_storages: DecodedPersistentId<ObjectDefinitionAtomId>,
    immortal_objects: DecodedPersistentId<ObjectDefinitionAtomId>,
    initialization_units: DecodedPersistentId<ObjectDefinitionAtomId>,
    type_registrations: DecodedPersistentId<ObjectDefinitionAtomId>,
    safepoints: DecodedPersistentId<ObjectDefinitionAtomId>,
    callables: DecodedPersistentId<ObjectDefinitionAtomId>,
    array_bounds_message: DecodedPersistentId<ObjectDefinitionAtomId>,
    array_size_overflow_message: DecodedPersistentId<ObjectDefinitionAtomId>,
}

impl WireEncode for DecodedConeImageSupportAtomsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_image_support_atoms(
            encoder,
            &self.coordinate_group,
            &self.coordinate_name,
            &self.coordinate_version,
            &self.dependencies,
            &self.static_storages,
            &self.immortal_objects,
            &self.initialization_units,
            &self.type_registrations,
            &self.safepoints,
            &self.callables,
            &self.array_bounds_message,
            &self.array_size_overflow_message,
        )
    }
}

impl WireDecode for DecodedConeImageSupportAtomsV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(12)?;
        Ok(Self {
            coordinate_group: decoder.field(1, DecodedPersistentId::decode)?,
            coordinate_name: decoder.field(2, DecodedPersistentId::decode)?,
            coordinate_version: decoder.field(3, DecodedPersistentId::decode)?,
            dependencies: decoder.field(4, DecodedPersistentId::decode)?,
            static_storages: decoder.field(5, DecodedPersistentId::decode)?,
            immortal_objects: decoder.field(6, DecodedPersistentId::decode)?,
            initialization_units: decoder.field(7, DecodedPersistentId::decode)?,
            type_registrations: decoder.field(8, DecodedPersistentId::decode)?,
            safepoints: decoder.field(9, DecodedPersistentId::decode)?,
            callables: decoder.field(10, DecodedPersistentId::decode)?,
            array_bounds_message: decoder.field(11, DecodedPersistentId::decode)?,
            array_size_overflow_message: decoder.field(12, DecodedPersistentId::decode)?,
        })
    }
}

pub(super) fn require_image_atoms(
    foundation: &OdrFreeLirFoundation,
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
        .definition_atoms()
        .iter()
        .filter(|record| record.key().plan() == definition)
        .map(|record| record.key().clone())
        .collect::<Vec<_>>();
    actual.sort_unstable();
    if actual != expected {
        return Err(ConeImagePlanBuildError::AtomSet { expected, actual });
    }

    let atom_id = |key: &ObjectDefinitionAtomKey| {
        foundation
            .definition_atoms()
            .iter()
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

#[allow(clippy::too_many_arguments)]
fn encode_image_support_atoms<A: WireEncode>(
    encoder: &mut Encoder,
    coordinate_group: &A,
    coordinate_name: &A,
    coordinate_version: &A,
    dependencies: &A,
    static_storages: &A,
    immortal_objects: &A,
    initialization_units: &A,
    type_registrations: &A,
    safepoints: &A,
    callables: &A,
    array_bounds_message: &A,
    array_size_overflow_message: &A,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(12)?;
    for (field, value) in [
        (1, coordinate_group),
        (2, coordinate_name),
        (3, coordinate_version),
        (4, dependencies),
        (5, static_storages),
        (6, immortal_objects),
        (7, initialization_units),
        (8, type_registrations),
        (9, safepoints),
        (10, callables),
        (11, array_bounds_message),
        (12, array_size_overflow_message),
    ] {
        encoder.field(field)?;
        value.encode(encoder)?;
    }
    Ok(())
}
