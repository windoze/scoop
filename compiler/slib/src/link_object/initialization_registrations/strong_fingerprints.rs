use std::fmt;

use scoop_identity::{DigestNodeId, PersistentCallableBodyId, PersistentInitializationUnitId};
use scoop_lir::StrongInitializationUnitRegistrationPlan;
use scoop_wire::{HashError, RuntimeEncodeError, RuntimeEncoder};

use super::InitializationRegistrationDataV1;
use crate::link_object::{
    ObjectDefinitionFingerprintV1, OdrMemberAbiV1, RegistrationAbiV1,
    VerifiedStrongCallableBodyObjectFingerprintSetV1,
};

const INITIALIZATION_RECORD_KIND: u32 = 3;
const OWN_INITIALIZATION_CELL_ROLE: u32 = 3;
const CALLABLE_ENTRY_ROLE: u32 = 4;
const UNIT_GATEWAY_ROLE: u32 = 5;
const NO_GATEWAY: u32 = 0;
const UNIT_GATEWAY: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InitializationFingerprintV1 {
    unit: PersistentInitializationUnitId,

    gateway: Option<GatewayDefinitionV1>,

    registration: RegistrationAbiV1,
}

impl InitializationFingerprintV1 {
    pub const fn unit(self) -> PersistentInitializationUnitId {
        self.unit
    }

    pub const fn gateway_body(self) -> Option<PersistentCallableBodyId> {
        match self.gateway {
            Some(gateway) => Some(gateway.body),
            None => None,
        }
    }

    pub const fn gateway_definition_node(self) -> Option<DigestNodeId> {
        match self.gateway {
            Some(gateway) => Some(gateway.node),
            None => None,
        }
    }

    pub const fn gateway_definition(self) -> Option<ObjectDefinitionFingerprintV1> {
        match self.gateway {
            Some(gateway) => Some(gateway.fingerprint),
            None => None,
        }
    }

    pub const fn registration(self) -> RegistrationAbiV1 {
        self.registration
    }
}

/// Actual gateway fingerprints and shared ABIs of initialization units.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitializationFingerprintSetV1<D = scoop_identity::PersistentInitializationUnitId> {
    registrations: InitializationRegistrationDataV1<D>,
    fingerprints: Vec<InitializationFingerprintV1>,
    odr_definitions: Vec<OdrMemberAbiV1>,
}

pub type InitializationFingerprintSetV2 =
    InitializationFingerprintSetV1<scoop_lir::StrongInitializationDependencyRefV2>;

impl<D> InitializationFingerprintSetV1<D> {
    pub fn odr_definitions(&self) -> &[OdrMemberAbiV1] {
        &self.odr_definitions
    }
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.registrations.producer()
    }

    pub const fn registrations(&self) -> &InitializationRegistrationDataV1<D> {
        &self.registrations
    }

    pub fn fingerprints(&self) -> &[InitializationFingerprintV1] {
        &self.fingerprints
    }
}

pub fn compute_strong_initialization_fingerprints_v1(
    registrations: InitializationRegistrationDataV1,
    callable_bodies: &VerifiedStrongCallableBodyObjectFingerprintSetV1,
    canonical: &scoop_lir::CanonicalShapeAbisV1,
) -> Result<InitializationFingerprintSetV1, StrongInitializationFingerprintError> {
    compute_strong_initialization_fingerprints(registrations, callable_bodies, canonical)
}

pub fn compute_strong_initialization_fingerprints_v2(
    registrations: super::InitializationRegistrationDataV1<
        scoop_lir::StrongInitializationDependencyRefV2,
    >,
    callable_bodies: &VerifiedStrongCallableBodyObjectFingerprintSetV1,
    canonical: &scoop_lir::CanonicalShapeAbisV1,
) -> Result<InitializationFingerprintSetV2, StrongInitializationFingerprintError> {
    compute_strong_initialization_fingerprints(registrations, callable_bodies, canonical)
}

fn compute_strong_initialization_fingerprints<D>(
    registrations: InitializationRegistrationDataV1<D>,
    callable_bodies: &VerifiedStrongCallableBodyObjectFingerprintSetV1,
    canonical: &scoop_lir::CanonicalShapeAbisV1,
) -> Result<InitializationFingerprintSetV1<D>, StrongInitializationFingerprintError> {
    let planned = registrations.registrations();
    let mut fingerprints = Vec::with_capacity(planned.len());
    let mut odr_definitions = Vec::new();
    for plan in planned {
        let unit = plan.semantic().unit();
        let gateway = gateway_definition(plan, callable_bodies)?;
        if let scoop_lir::RegistrationDefinitionOwner::Odr { group, .. } = plan.definition_owner() {
            let content = canonical
                .definitions()
                .iter()
                .find(|content| {
                    content.definition() == plan.cell_definition_plan()
                        && content.group() == group
                        && content.role() == scoop_identity::OdrMemberRole::InitializationCell
                })
                .ok_or(StrongInitializationFingerprintError::DefinitionMismatch { unit })?;
            odr_definitions.push(super::super::odr_member_fingerprints::shape_definition(
                *content,
            ));
        }
        let registration = RegistrationAbiV1::from_owner(
            plan.definition_owner(),
            scoop_lir::RegistrationTableV1::InitializationUnit,
        )
        .map_err(|source| StrongInitializationFingerprintError::Hash { unit, source })?;
        fingerprints.push(InitializationFingerprintV1 {
            unit,

            gateway,

            registration,
        });
    }

    Ok(InitializationFingerprintSetV1 {
        registrations,
        fingerprints,
        odr_definitions,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
    Ok(Some(GatewayDefinitionV1 {
        body: fingerprint.body(),
        node: fingerprint.node(),
        fingerprint: fingerprint.fingerprint(),
    }))
}

pub(in crate::link_object) fn runtime_encode_strong_initialization_record_v1<D>(
    encoder: &mut RuntimeEncoder,
    plan: &StrongInitializationUnitRegistrationPlan<D>,
    gateway: Option<(PersistentCallableBodyId, ObjectDefinitionFingerprintV1)>,
) -> Result<(), RuntimeEncodeError> {
    let semantic = plan.semantic();
    encoder.u32(INITIALIZATION_RECORD_KIND)?;
    super::super::registration_identity::runtime_encode_registration_identity(
        encoder,
        semantic.unit().as_array(),
        plan.definition_owner(),
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
    DefinitionMismatch {
        unit: PersistentInitializationUnitId,
    },
    MissingGatewayBody {
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
