use std::fmt;

use scoop_identity::{
    DigestKind, DigestNodeId, PersistentCallableBodyId, PersistentInitializationUnitId,
};
use scoop_lir::StrongInitializationUnitRegistrationPlan;
use scoop_wire::{
    HashError, RuntimeEncode, RuntimeEncodeError, RuntimeEncoder, domain_separated_runtime_hash,
};

use super::definition_fingerprints::VerifiedStrongInitializationDefinitionFingerprintSetV1;
use crate::link_object::callable_registrations::object_definition::CanonicalDigestInputV1;
use crate::link_object::{
    ObjectDefinitionFingerprintV1, OdrMemberFingerprintV1, RegistrationFingerprintV1,
    StrongRegistrationFingerprintV1, VerifiedStrongCallableBodyObjectFingerprintSetV1,
};

const STRONG_REGISTRATION_DOMAIN: &str = "scoop-strong-registration-v1";
const INITIALIZATION_RECORD_KIND: u32 = 3;
const OWN_INITIALIZATION_CELL_ROLE: u32 = 3;
const CALLABLE_ENTRY_ROLE: u32 = 4;
const UNIT_GATEWAY_ROLE: u32 = 5;
const NO_GATEWAY: u32 = 0;
const UNIT_GATEWAY: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedStrongInitializationFingerprintV1 {
    unit: PersistentInitializationUnitId,
    registration_object_node: DigestNodeId,
    registration_object: ObjectDefinitionFingerprintV1,
    cell_definition_node: DigestNodeId,
    cell_definition: ObjectDefinitionFingerprintV1,
    gateway_body: Option<PersistentCallableBodyId>,
    gateway_definition_node: Option<DigestNodeId>,
    gateway_definition: Option<ObjectDefinitionFingerprintV1>,
    registration_node: DigestNodeId,
    registration: RegistrationFingerprintV1,
}

impl VerifiedStrongInitializationFingerprintV1 {
    pub const fn unit(self) -> PersistentInitializationUnitId {
        self.unit
    }

    pub const fn registration_object_node(self) -> DigestNodeId {
        self.registration_object_node
    }

    pub const fn registration_object(self) -> ObjectDefinitionFingerprintV1 {
        self.registration_object
    }

    pub const fn cell_definition_node(self) -> DigestNodeId {
        self.cell_definition_node
    }

    pub const fn cell_definition(self) -> ObjectDefinitionFingerprintV1 {
        self.cell_definition
    }

    pub const fn gateway_body(self) -> Option<PersistentCallableBodyId> {
        self.gateway_body
    }

    pub const fn gateway_definition_node(self) -> Option<DigestNodeId> {
        self.gateway_definition_node
    }

    pub const fn gateway_definition(self) -> Option<ObjectDefinitionFingerprintV1> {
        self.gateway_definition
    }

    pub const fn registration_node(self) -> DigestNodeId {
        self.registration_node
    }

    pub const fn registration(self) -> RegistrationFingerprintV1 {
        self.registration
    }
}

/// Canonical initialization-unit strong-registration fingerprints derived
/// from exact initialization leaves and the shared callable-body proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedStrongInitializationFingerprintSetV1<
    D = scoop_identity::PersistentInitializationUnitId,
> {
    definitions: VerifiedStrongInitializationDefinitionFingerprintSetV1<D>,
    fingerprints: Vec<VerifiedStrongInitializationFingerprintV1>,
    odr_definitions: Vec<OdrMemberFingerprintV1>,
}

pub type VerifiedStrongInitializationFingerprintSetV2 =
    VerifiedStrongInitializationFingerprintSetV1<scoop_lir::StrongInitializationDependencyRefV2>;

impl<D> VerifiedStrongInitializationFingerprintSetV1<D> {
    pub fn odr_definitions(&self) -> &[OdrMemberFingerprintV1] {
        &self.odr_definitions
    }
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.definitions.producer()
    }

    pub const fn definitions(&self) -> &VerifiedStrongInitializationDefinitionFingerprintSetV1<D> {
        &self.definitions
    }

    pub fn fingerprints(&self) -> &[VerifiedStrongInitializationFingerprintV1] {
        &self.fingerprints
    }
}

pub fn compute_strong_initialization_fingerprints_v1(
    definitions: VerifiedStrongInitializationDefinitionFingerprintSetV1,
    callable_bodies: &VerifiedStrongCallableBodyObjectFingerprintSetV1,
    canonical: &scoop_lir::CanonicalShapeLirDefinitionsV1,
) -> Result<VerifiedStrongInitializationFingerprintSetV1, StrongInitializationFingerprintError> {
    compute_strong_initialization_fingerprints(definitions, callable_bodies, canonical)
}

pub fn compute_strong_initialization_fingerprints_v2(
    definitions: super::VerifiedStrongInitializationDefinitionFingerprintSetV2,
    callable_bodies: &VerifiedStrongCallableBodyObjectFingerprintSetV1,
    canonical: &scoop_lir::CanonicalShapeLirDefinitionsV1,
) -> Result<VerifiedStrongInitializationFingerprintSetV2, StrongInitializationFingerprintError> {
    compute_strong_initialization_fingerprints(definitions, callable_bodies, canonical)
}

fn compute_strong_initialization_fingerprints<D>(
    definitions: VerifiedStrongInitializationDefinitionFingerprintSetV1<D>,
    callable_bodies: &VerifiedStrongCallableBodyObjectFingerprintSetV1,
    canonical: &scoop_lir::CanonicalShapeLirDefinitionsV1,
) -> Result<VerifiedStrongInitializationFingerprintSetV1<D>, StrongInitializationFingerprintError> {
    let registration_objects = definitions.registration_objects();
    let registrations = registration_objects.registrations();
    let callable_registrations = callable_bodies.registration_objects().registrations();
    if registrations.patch_sites() != callable_registrations.patch_sites() {
        return Err(StrongInitializationFingerprintError::CallableProofMismatch);
    }
    let verified = registrations.registrations();
    let planned = registrations.plan().registrations();
    let registration_fingerprints = registration_objects.fingerprints();
    let definition_fingerprints = definitions.fingerprints();
    if verified.len() != planned.len()
        || verified.len() != registration_fingerprints.len()
        || verified.len() != definition_fingerprints.len()
    {
        return Err(StrongInitializationFingerprintError::ProofCoverageMismatch);
    }

    let mut fingerprints = Vec::with_capacity(verified.len());
    let mut odr_definitions = Vec::new();
    for (((verified, plan), registration_object), definition) in verified
        .iter()
        .zip(planned)
        .zip(registration_fingerprints)
        .zip(definition_fingerprints)
    {
        let unit = plan.semantic().unit();
        if verified.unit() != unit
            || registration_object.unit() != unit
            || registration_object.node() != plan.registration_object_node()
        {
            return Err(StrongInitializationFingerprintError::RegistrationObjectMismatch { unit });
        }
        if definition.unit() != unit || definition.cell_node() != plan.cell_definition_node() {
            return Err(StrongInitializationFingerprintError::DefinitionMismatch { unit });
        }
        let gateway = gateway_definition(plan, callable_bodies)?;
        let registration = match plan.definition_owner() {
            scoop_lir::RegistrationDefinitionOwner::Strong => strong_initialization_fingerprint(
                plan,
                registration_object.node(),
                registration_object.fingerprint(),
                definition.cell_node(),
                definition.cell(),
                gateway,
            )
            .map(RegistrationFingerprintV1::Strong),
            scoop_lir::RegistrationDefinitionOwner::Odr { group, member } => {
                if gateway.is_some() {
                    return Err(StrongInitializationFingerprintError::DefinitionMismatch { unit });
                }
                let content = canonical
                    .definitions()
                    .iter()
                    .find(|content| {
                        content.definition() == plan.cell_definition_plan()
                            && content.group() == group
                            && content.role() == scoop_identity::OdrMemberRole::InitializationCell
                    })
                    .ok_or(StrongInitializationFingerprintError::DefinitionMismatch { unit })?;
                odr_definitions.push(
                    super::super::odr_member_fingerprints::shape_definition(
                        *content,
                        definition.cell(),
                    )
                    .map_err(|source| {
                        StrongInitializationFingerprintError::Hash { unit, source }
                    })?,
                );
                super::super::odr_member_fingerprints::initialization_registration(
                    group,
                    member,
                    plan,
                    registration_object.fingerprint(),
                    definition.cell(),
                )
                .map(RegistrationFingerprintV1::Odr)
            }
        }
        .map_err(|source| StrongInitializationFingerprintError::Hash { unit, source })?;
        fingerprints.push(VerifiedStrongInitializationFingerprintV1 {
            unit,
            registration_object_node: registration_object.node(),
            registration_object: registration_object.fingerprint(),
            cell_definition_node: definition.cell_node(),
            cell_definition: definition.cell(),
            gateway_body: gateway.map(|gateway| gateway.body),
            gateway_definition_node: gateway.map(|gateway| gateway.node),
            gateway_definition: gateway.map(|gateway| gateway.fingerprint),
            registration_node: plan.registration_fingerprint_node(),
            registration,
        });
    }

    Ok(VerifiedStrongInitializationFingerprintSetV1 {
        definitions,
        fingerprints,
        odr_definitions,
    })
}

#[derive(Clone, Copy)]
struct GatewayDefinitionV1 {
    body: PersistentCallableBodyId,
    node: DigestNodeId,
    fingerprint: ObjectDefinitionFingerprintV1,
}

fn gateway_definition<D>(
    plan: &StrongInitializationUnitRegistrationPlan<D>,
    callable_bodies: &VerifiedStrongCallableBodyObjectFingerprintSetV1,
) -> Result<Option<GatewayDefinitionV1>, StrongInitializationFingerprintError> {
    let Some(gateway) = plan.schedule().gateway() else {
        return Ok(None);
    };
    let Some(fingerprint) = callable_bodies
        .fingerprints()
        .iter()
        .find(|fingerprint| fingerprint.body() == gateway.body())
        .copied()
    else {
        return Err(StrongInitializationFingerprintError::MissingGatewayBody {
            unit: plan.semantic().unit(),
            body: gateway.body(),
        });
    };
    if fingerprint.node() != gateway.body_definition_node() {
        return Err(StrongInitializationFingerprintError::GatewayBodyMismatch {
            unit: plan.semantic().unit(),
            body: gateway.body(),
        });
    }
    Ok(Some(GatewayDefinitionV1 {
        body: fingerprint.body(),
        node: fingerprint.node(),
        fingerprint: fingerprint.fingerprint(),
    }))
}

#[allow(clippy::too_many_arguments)]
fn strong_initialization_fingerprint<D>(
    plan: &StrongInitializationUnitRegistrationPlan<D>,
    registration_object_node: DigestNodeId,
    registration_object: ObjectDefinitionFingerprintV1,
    cell_definition_node: DigestNodeId,
    cell_definition: ObjectDefinitionFingerprintV1,
    gateway: Option<GatewayDefinitionV1>,
) -> Result<StrongRegistrationFingerprintV1, HashError> {
    let mut direct_inputs = vec![
        CanonicalDigestInputV1 {
            kind: DigestKind::ObjectDefinition,
            node: registration_object_node,
            digest: *registration_object.as_array(),
        },
        CanonicalDigestInputV1 {
            kind: DigestKind::ObjectDefinition,
            node: cell_definition_node,
            digest: *cell_definition.as_array(),
        },
    ];
    if let Some(gateway) = gateway {
        direct_inputs.push(CanonicalDigestInputV1 {
            kind: DigestKind::ObjectDefinition,
            node: gateway.node,
            digest: *gateway.fingerprint.as_array(),
        });
    }
    direct_inputs.sort_unstable_by_key(|input| (input.kind.tag(), input.node));
    domain_separated_runtime_hash(
        STRONG_REGISTRATION_DOMAIN,
        &StrongInitializationFingerprintInputV1 {
            plan,
            gateway,
            direct_inputs: &direct_inputs,
        },
    )
    .map(|digest| StrongRegistrationFingerprintV1::from_array(*digest.as_array()))
}

struct StrongInitializationFingerprintInputV1<'a, D> {
    plan: &'a StrongInitializationUnitRegistrationPlan<D>,
    gateway: Option<GatewayDefinitionV1>,
    direct_inputs: &'a [CanonicalDigestInputV1],
}

impl<D> RuntimeEncode for StrongInitializationFingerprintInputV1<'_, D> {
    fn runtime_encode(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        runtime_encode_strong_initialization_record_v1(
            encoder,
            self.plan,
            &[0; 32],
            self.gateway
                .map(|gateway| (gateway.body, gateway.fingerprint)),
        )?;
        encoder.sequence_length(self.direct_inputs.len())?;
        for input in self.direct_inputs {
            input.runtime_encode(encoder)?;
        }
        Ok(())
    }
}

pub(in crate::link_object) fn runtime_encode_strong_initialization_record_v1<D>(
    encoder: &mut RuntimeEncoder,
    plan: &StrongInitializationUnitRegistrationPlan<D>,
    registration: &[u8; 32],
    gateway: Option<(PersistentCallableBodyId, ObjectDefinitionFingerprintV1)>,
) -> Result<(), RuntimeEncodeError> {
    let semantic = plan.semantic();
    encoder.u32(INITIALIZATION_RECORD_KIND)?;
    super::super::registration_identity::runtime_encode_registration_identity(
        encoder,
        semantic.unit().as_array(),
        plan.definition_owner(),
        registration,
    )?;
    encoder.u32(semantic.schedule().tag())?;
    encoder.byte_span(semantic.diagnostic_path().as_bytes())?;
    encoder.u32(OWN_INITIALIZATION_CELL_ROLE)?;
    encoder.fixed(semantic.unit().as_array())?;
    encoder.fixed(semantic.storage().as_array())?;
    encoder.fixed(semantic.failure_root().as_array())?;
    encoder.u32(CALLABLE_ENTRY_ROLE)?;
    encoder.fixed(semantic.initializer().as_array())?;
    encoder.u32(CALLABLE_ENTRY_ROLE)?;
    encoder.fixed(semantic.ensure().as_array())?;
    match gateway {
        None => encoder.u32(NO_GATEWAY)?,
        Some((body, fingerprint)) => {
            encoder.u32(UNIT_GATEWAY)?;
            encoder.u32(UNIT_GATEWAY_ROLE)?;
            encoder.fixed(body.as_array())?;
            encoder.fixed(fingerprint.as_array())?;
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongInitializationFingerprintError {
    CallableProofMismatch,
    ProofCoverageMismatch,
    RegistrationObjectMismatch {
        unit: PersistentInitializationUnitId,
    },
    DefinitionMismatch {
        unit: PersistentInitializationUnitId,
    },
    MissingGatewayBody {
        unit: PersistentInitializationUnitId,
        body: PersistentCallableBodyId,
    },
    GatewayBodyMismatch {
        unit: PersistentInitializationUnitId,
        body: PersistentCallableBodyId,
    },
    Hash {
        unit: PersistentInitializationUnitId,
        source: HashError,
    },
}

impl fmt::Display for StrongInitializationFingerprintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to compute strong initialization fingerprints: {self:?}"
        )
    }
}

impl std::error::Error for StrongInitializationFingerprintError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Hash { source, .. } => Some(source),
            _ => None,
        }
    }
}
