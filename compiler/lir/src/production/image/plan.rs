//! Complete image plans with explicit artifact dependencies.

use super::*;

#[cfg(test)]
#[path = "tests/dependencies.rs"]
mod dependency_tests;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConeImagePlanV1 {
    cone: ConeRecordV1,
    dependencies: Vec<ConeIdentity>,
    tables: ConeRegistrationTablesV1,
    symbol: PersistentSymbolRequest,
    definition_plan: ObjectDefinitionPlanId,
    primary_atom: ObjectDefinitionAtomId,
    support_atoms: ConeImageSupportAtomsV1,
    fingerprint_patch: DigestPatchIntentId,
}

impl ConeImagePlanV1 {
    pub fn new(
        coordinate: ConeCoordinate,
        direct_dependencies: &[ConeIdentity],
        foundation: &ConeLirFoundation,
        registrations: &RegistrationIdentitySurfaceV1,
        digest_plan: &DigestFinalizationPlanV1,
    ) -> Result<Self, ConeImagePlanBuildError> {
        let cone = ConeRecordV1::new(coordinate).map_err(ConeImagePlanBuildError::Hash)?;
        if cone.identity() != foundation.producer() {
            return Err(ConeImagePlanBuildError::ProducerMismatch {
                coordinate: cone.identity(),
                foundation: foundation.producer(),
            });
        }
        let dependencies = dependencies::canonical(cone.identity(), direct_dependencies)?;
        let tables = ConeRegistrationTablesV1::from_registrations(registrations);
        let symbol = PersistentSymbolRequest::new(
            PersistentSymbolKey::ImageDescriptor(cone.identity()),
            LinkageClass::ConeStrong,
        )
        .map_err(ConeImagePlanBuildError::Symbol)?;
        if !foundation.contains_symbol_request(symbol) {
            return Err(ConeImagePlanBuildError::MissingSymbol(symbol));
        }
        let definition_key = ObjectDefinitionPlanKey::strong(
            cone.identity(),
            StrongDefinitionEntity::cone_image(cone.identity()),
            StrongDefinitionRole::ImageDescriptor,
        )
        .map_err(ConeImagePlanBuildError::Definition)?;
        let definition_plan = ObjectDefinitionPlanId::from_key(&definition_key)
            .map_err(ConeImagePlanBuildError::Hash)?;
        if !foundation
            .definition_plans()
            .iter()
            .any(|record| record.id() == definition_plan && record.key() == &definition_key)
        {
            return Err(ConeImagePlanBuildError::MissingDefinition(definition_plan));
        }
        let (primary_atom, support_atoms) = require_image_atoms(foundation, definition_plan)?;

        let image_node = digest_plan
            .nodes()
            .iter()
            .find(|node| {
                node.key().owner_and_role()
                    == scoop_identity::DigestOwnerAndRoleKey::RuntimeImage(cone.identity())
            })
            .ok_or(ConeImagePlanBuildError::MissingDigestNode)?;
        validate_registration_inputs(image_node.direct_inputs(), registrations)?;
        let patch_key = DigestPatchIntentKey::new(
            image_node.id(),
            definition_plan,
            DefinitionAtomRole::Primary,
            DigestSemanticFieldRole::RuntimeImage,
        );
        let fingerprint_patch =
            DigestPatchIntentId::from_key(&patch_key).map_err(ConeImagePlanBuildError::Hash)?;
        if !image_node
            .patch_intents()
            .iter()
            .any(|record| record.id() == fingerprint_patch && record.key() == &patch_key)
        {
            return Err(ConeImagePlanBuildError::MissingFingerprintPatch(
                fingerprint_patch,
            ));
        }

        Ok(Self {
            cone,
            dependencies,
            tables,
            symbol,
            definition_plan,
            primary_atom,
            support_atoms,
            fingerprint_patch,
        })
    }

    pub const fn cone(&self) -> &ConeRecordV1 {
        &self.cone
    }

    pub fn dependencies(&self) -> &[ConeIdentity] {
        &self.dependencies
    }

    pub const fn tables(&self) -> &ConeRegistrationTablesV1 {
        &self.tables
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

    pub const fn support_atoms(&self) -> ConeImageSupportAtomsV1 {
        self.support_atoms
    }

    pub const fn fingerprint_patch(&self) -> DigestPatchIntentId {
        self.fingerprint_patch
    }
}

impl WireEncode for ConeImagePlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(7)?;
        encoder.field(1)?;
        self.cone.encode(encoder)?;
        encoder.field(2)?;
        encode_array(encoder, &self.dependencies)?;
        encoder.field(3)?;
        self.tables.encode(encoder)?;
        encoder.field(4)?;
        self.symbol.encode(encoder)?;
        encoder.field(5)?;
        self.definition_plan.encode(encoder)?;
        encoder.field(6)?;
        self.support_atoms.encode(encoder)?;
        encoder.field(7)?;
        self.fingerprint_patch.encode(encoder)
    }
}

#[derive(Debug)]
pub struct DecodedConeImagePlanV1 {
    cone: DecodedConeRecordV1,
    dependencies: Vec<DecodedPersistentId<ConeIdentity>>,
    tables: DecodedConeRegistrationTablesV1,
    symbol: DecodedPersistentSymbolRequest,
    definition_plan: DecodedPersistentId<ObjectDefinitionPlanId>,
    support_atoms: DecodedConeImageSupportAtomsV1,
    fingerprint_patch: DecodedPersistentId<DigestPatchIntentId>,
}

impl DecodedConeImagePlanV1 {
    pub fn validate(
        self,
        coordinate: &ConeCoordinate,
        direct_dependencies: &[ConeIdentity],
        foundation: &ConeLirFoundation,
        registrations: &RegistrationIdentitySurfaceV1,
        digest_plan: &DigestFinalizationPlanV1,
    ) -> Result<ConeImagePlanV1, ConeImagePlanValidationError> {
        let actual = encode(&self).map_err(ConeImagePlanValidationError::Encode)?;
        let decoded_coordinate = self
            .cone
            .coordinate
            .validate()
            .map_err(ConeImagePlanValidationError::Coordinate)?;
        if &decoded_coordinate != coordinate {
            return Err(ConeImagePlanValidationError::CoordinateMismatch);
        }
        let expected_identity = coordinate
            .identity()
            .map_err(ConeImagePlanValidationError::Hash)?;
        self.cone
            .identity
            .verify(expected_identity)
            .map_err(ConeImagePlanValidationError::Identity)?;
        let expected = ConeImagePlanV1::new(
            coordinate.clone(),
            direct_dependencies,
            foundation,
            registrations,
            digest_plan,
        )
        .map_err(ConeImagePlanValidationError::Expected)?;
        let expected_bytes = encode(&expected).map_err(ConeImagePlanValidationError::Encode)?;
        if actual != expected_bytes {
            return Err(ConeImagePlanValidationError::PlanMismatch);
        }
        Ok(expected)
    }
}

impl WireEncode for DecodedConeImagePlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(7)?;
        encoder.field(1)?;
        self.cone.encode(encoder)?;
        encoder.field(2)?;
        encode_array(encoder, &self.dependencies)?;
        encoder.field(3)?;
        self.tables.encode(encoder)?;
        encoder.field(4)?;
        self.symbol.encode(encoder)?;
        encoder.field(5)?;
        self.definition_plan.encode(encoder)?;
        encoder.field(6)?;
        self.support_atoms.encode(encoder)?;
        encoder.field(7)?;
        self.fingerprint_patch.encode(encoder)
    }
}

impl WireDecode for DecodedConeImagePlanV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(7)?;
        Ok(Self {
            cone: decoder.field(1, DecodedConeRecordV1::decode)?,
            dependencies: decode_ids(decoder, 2)?,
            tables: decoder.field(3, DecodedConeRegistrationTablesV1::decode)?,
            symbol: decoder.field(4, DecodedPersistentSymbolRequest::decode)?,
            definition_plan: decoder.field(5, DecodedPersistentId::decode)?,
            support_atoms: decoder.field(6, DecodedConeImageSupportAtomsV1::decode)?,
            fingerprint_patch: decoder.field(7, DecodedPersistentId::decode)?,
        })
    }
}
