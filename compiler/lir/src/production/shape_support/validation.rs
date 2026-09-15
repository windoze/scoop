use std::fmt;

use scoop_identity::{
    DefinitionAtomRole, ExactTypeKey, GeneratedNominalIdentityError, GeneratedNominalKey,
    LinkageClass, ObjectDefinitionAtomId, ObjectDefinitionIdentityError, ObjectDefinitionPlanId,
    ObjectDefinitionPlanKey, PersistentExactTypeId, PersistentId, PersistentLayoutId,
    PersistentScanId, PersistentSymbolError, PersistentSymbolKey, PersistentSymbolRequest,
    PersistentTypeId, RepresentationRole, ScanRole, SourceDeclarationIdentityError,
    SourceDeclarationKey, SourceDeclarationKind, StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_wire::{HashError, cbor::EncodeError};

use super::{
    ClosedShapeSupportReasonV1, ParamFreeShapeSupportClosureV1, ParamFreeShapeSupportRolesV1,
    ShapeSupportAvailabilityV1, StrongExactShapeSupportV1, StrongShapeDefinitionV1,
    StrongShapeRegistrationV1,
};
use crate::{OdrFreeLirFoundation, StrongRegistrationIdentitySurfaceV1};

pub(super) fn build_closure(
    source: &SourceDeclarationKey,
    foundation: &OdrFreeLirFoundation,
    registrations: &StrongRegistrationIdentitySurfaceV1,
) -> Result<ParamFreeShapeSupportClosureV1, ParamFreeShapeSupportBuildError> {
    validate_source(source)?;
    let source_nominal = PersistentTypeId::from_source_declaration(source)
        .map_err(ParamFreeShapeSupportBuildError::SourceIdentity)?;
    let owner = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(source_nominal))
        .map_err(ParamFreeShapeSupportBuildError::Hash)?;
    let owner_support = exact_shape_support(source_nominal, foundation, registrations)?;
    if owner_support.exact != owner {
        return Err(ParamFreeShapeSupportBuildError::ExactTypeMismatch {
            exact: owner,
            expected_nominal: source_nominal,
        });
    }

    let boxed_value = match source.declaration_kind() {
        SourceDeclarationKind::Struct | SourceDeclarationKind::Enum => {
            ShapeSupportAvailabilityV1::Available(generated_exact_support(
                GeneratedNominalKey::BoxedValue { payload: owner },
                foundation,
                registrations,
            )?)
        }
        SourceDeclarationKind::Class
        | SourceDeclarationKind::Interface
        | SourceDeclarationKind::Object
        | SourceDeclarationKind::AnnotationClass => ShapeSupportAvailabilityV1::NotApplicable(
            ClosedShapeSupportReasonV1::ReferenceNominalRequiresNoBox,
        ),
        _ => return Err(ParamFreeShapeSupportBuildError::NonNominalSource),
    };

    let coroutine_step = generated_exact_support(
        GeneratedNominalKey::CoroutineStep { result: owner },
        foundation,
        registrations,
    )?;
    let coroutine_slot = generated_exact_support(
        GeneratedNominalKey::CoroutineSlot { value: owner },
        foundation,
        registrations,
    )?;
    Ok(ParamFreeShapeSupportClosureV1 {
        owner,
        root: scoop_identity::ConeIdentity::CORE,
        roles: ParamFreeShapeSupportRolesV1 {
            source_nominal: ShapeSupportAvailabilityV1::Available(source_nominal),
            value_layout: ShapeSupportAvailabilityV1::Available(owner_support.layout),
            ref_scan: ShapeSupportAvailabilityV1::Available(owner_support.scan),
            type_descriptor: ShapeSupportAvailabilityV1::Available(owner_support.descriptor),
            type_registration: ShapeSupportAvailabilityV1::Available(owner_support.registration),
            boxed_value,
            coroutine_step: ShapeSupportAvailabilityV1::Available(coroutine_step),
            coroutine_slot: ShapeSupportAvailabilityV1::Available(coroutine_slot),
        },
    })
}

fn validate_source(source: &SourceDeclarationKey) -> Result<(), ParamFreeShapeSupportBuildError> {
    if source.origin() != scoop_identity::ConeIdentity::CORE {
        return Err(ParamFreeShapeSupportBuildError::ForeignSource(
            source.origin(),
        ));
    }
    if !source.declaration_kind().is_nominal() {
        return Err(ParamFreeShapeSupportBuildError::NonNominalSource);
    }
    if source.duplicate_signature().type_parameter_count() != 0 {
        return Err(ParamFreeShapeSupportBuildError::GenericSource);
    }
    Ok(())
}

fn generated_exact_support(
    key: GeneratedNominalKey,
    foundation: &OdrFreeLirFoundation,
    registrations: &StrongRegistrationIdentitySurfaceV1,
) -> Result<StrongExactShapeSupportV1, ParamFreeShapeSupportBuildError> {
    let nominal = PersistentTypeId::from_generated_key(&key)
        .map_err(ParamFreeShapeSupportBuildError::GeneratedNominalIdentity)?;
    exact_shape_support(nominal, foundation, registrations)
}

fn exact_shape_support(
    nominal: PersistentTypeId,
    foundation: &OdrFreeLirFoundation,
    registrations: &StrongRegistrationIdentitySurfaceV1,
) -> Result<StrongExactShapeSupportV1, ParamFreeShapeSupportBuildError> {
    let exact = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(nominal))
        .map_err(ParamFreeShapeSupportBuildError::Hash)?;
    if !foundation.materialized_exact_types().contains(&exact) {
        return Err(ParamFreeShapeSupportBuildError::MissingExactType(exact));
    }

    let layouts = foundation
        .layouts()
        .iter()
        .filter(|record| {
            record.key().exact_type() == exact
                && record.key().representation() == RepresentationRole::ManagedValue
        })
        .collect::<Vec<_>>();
    let layout = match layouts.as_slice() {
        [record] => record.id(),
        _ => {
            return Err(ParamFreeShapeSupportBuildError::ManagedValueLayoutSet {
                exact,
                actual: layouts.iter().map(|record| record.id()).collect(),
            });
        }
    };
    let scans = foundation
        .scans()
        .iter()
        .filter(|record| {
            record.key().layout() == layout && record.key().role() == ScanRole::InlineValue
        })
        .collect::<Vec<_>>();
    let scan = match scans.as_slice() {
        [record] => record.id(),
        _ => {
            return Err(ParamFreeShapeSupportBuildError::InlineValueScanSet {
                layout,
                actual: scans.iter().map(|record| record.id()).collect(),
            });
        }
    };

    Ok(StrongExactShapeSupportV1 {
        nominal,
        exact,
        layout: strong_definition(
            layout,
            foundation,
            StrongDefinitionEntity::layout(layout),
            StrongDefinitionRole::Layout,
            PersistentSymbolKey::Layout(layout),
        )?,
        scan: strong_definition(
            scan,
            foundation,
            StrongDefinitionEntity::scan(scan),
            StrongDefinitionRole::ScanProgram,
            PersistentSymbolKey::ScanProgram(scan),
        )?,
        descriptor: strong_definition(
            exact,
            foundation,
            StrongDefinitionEntity::exact_type(exact),
            StrongDefinitionRole::TypeDescriptor,
            PersistentSymbolKey::TypeDescriptor(exact),
        )?,
        registration: strong_registration(
            exact,
            foundation,
            registrations.type_registrations(),
            StrongDefinitionEntity::exact_type(exact),
            StrongDefinitionRole::TypeRegistration,
            PersistentSymbolKey::TypeRegistration(exact),
        )?,
    })
}

fn strong_definition<I: PersistentId>(
    semantic_id: I,
    foundation: &OdrFreeLirFoundation,
    entity: StrongDefinitionEntity,
    role: StrongDefinitionRole,
    symbol_key: PersistentSymbolKey,
) -> Result<StrongShapeDefinitionV1<I>, ParamFreeShapeSupportBuildError> {
    let definition_plan = require_definition(foundation, entity, role)?;
    require_primary_atom(foundation, definition_plan)?;
    let symbol = require_symbol(foundation, symbol_key)?;
    Ok(StrongShapeDefinitionV1 {
        semantic_id,
        definition_plan,
        symbol,
    })
}

fn strong_registration<I: PersistentId>(
    semantic_id: I,
    foundation: &OdrFreeLirFoundation,
    registrations: &[crate::StrongRegistrationIdentityV1<I>],
    entity: StrongDefinitionEntity,
    role: StrongDefinitionRole,
    symbol_key: PersistentSymbolKey,
) -> Result<StrongShapeRegistrationV1<I>, ParamFreeShapeSupportBuildError> {
    let definition_plan = require_definition(foundation, entity, role)?;
    require_primary_atom(foundation, definition_plan)?;
    let symbol = require_symbol(foundation, symbol_key)?;
    let registration = registrations
        .iter()
        .find(|entry| entry.semantic_id() == semantic_id)
        .ok_or(ParamFreeShapeSupportBuildError::MissingRegistration {
            kind: I::KIND,
            id: *semantic_id.as_array(),
        })?;
    if registration.definition_plan() != definition_plan {
        return Err(
            ParamFreeShapeSupportBuildError::RegistrationDefinitionMismatch {
                kind: I::KIND,
                id: *semantic_id.as_array(),
                expected: definition_plan,
                actual: registration.definition_plan(),
            },
        );
    }
    Ok(StrongShapeRegistrationV1 {
        semantic_id,
        definition_plan,
        symbol,
        fingerprint_node: registration.fingerprint_node(),
    })
}

fn require_definition(
    foundation: &OdrFreeLirFoundation,
    entity: StrongDefinitionEntity,
    role: StrongDefinitionRole,
) -> Result<ObjectDefinitionPlanId, ParamFreeShapeSupportBuildError> {
    let key = ObjectDefinitionPlanKey::strong(scoop_identity::ConeIdentity::CORE, entity, role)
        .map_err(ParamFreeShapeSupportBuildError::DefinitionIdentity)?;
    let id =
        ObjectDefinitionPlanId::from_key(&key).map_err(ParamFreeShapeSupportBuildError::Hash)?;
    foundation
        .definition_plans()
        .iter()
        .any(|record| record.id() == id && record.key() == &key)
        .then_some(id)
        .ok_or(ParamFreeShapeSupportBuildError::MissingDefinition(id))
}

fn require_primary_atom(
    foundation: &OdrFreeLirFoundation,
    plan: ObjectDefinitionPlanId,
) -> Result<ObjectDefinitionAtomId, ParamFreeShapeSupportBuildError> {
    let atoms = foundation
        .definition_atoms()
        .iter()
        .filter(|record| {
            record.key().plan() == plan && record.key().role() == DefinitionAtomRole::Primary
        })
        .map(|record| record.id())
        .collect::<Vec<_>>();
    match atoms.as_slice() {
        [atom] => Ok(*atom),
        _ => Err(ParamFreeShapeSupportBuildError::PrimaryAtomSet {
            plan,
            actual: atoms,
        }),
    }
}

fn require_symbol(
    foundation: &OdrFreeLirFoundation,
    key: PersistentSymbolKey,
) -> Result<PersistentSymbolRequest, ParamFreeShapeSupportBuildError> {
    let request = PersistentSymbolRequest::new(key, LinkageClass::ConeStrong)
        .map_err(ParamFreeShapeSupportBuildError::Symbol)?;
    foundation
        .contains_symbol_request(request)
        .then_some(request)
        .ok_or(ParamFreeShapeSupportBuildError::MissingSymbol(request))
}

#[derive(Debug)]
pub enum ParamFreeShapeSupportBuildError {
    SourceIdentity(SourceDeclarationIdentityError),
    GeneratedNominalIdentity(GeneratedNominalIdentityError),
    DefinitionIdentity(ObjectDefinitionIdentityError),
    Symbol(PersistentSymbolError),
    Hash(HashError),
    ProducerNotCore(scoop_identity::ConeIdentity),
    ForeignSource(scoop_identity::ConeIdentity),
    NonNominalSource,
    GenericSource,
    DuplicateOwner(PersistentExactTypeId),
    MissingExactType(PersistentExactTypeId),
    ExactTypeMismatch {
        exact: PersistentExactTypeId,
        expected_nominal: PersistentTypeId,
    },
    ManagedValueLayoutSet {
        exact: PersistentExactTypeId,
        actual: Vec<PersistentLayoutId>,
    },
    InlineValueScanSet {
        layout: PersistentLayoutId,
        actual: Vec<PersistentScanId>,
    },
    MissingDefinition(ObjectDefinitionPlanId),
    PrimaryAtomSet {
        plan: ObjectDefinitionPlanId,
        actual: Vec<ObjectDefinitionAtomId>,
    },
    MissingSymbol(PersistentSymbolRequest),
    MissingRegistration {
        kind: &'static str,
        id: [u8; 32],
    },
    RegistrationDefinitionMismatch {
        kind: &'static str,
        id: [u8; 32],
        expected: ObjectDefinitionPlanId,
        actual: ObjectDefinitionPlanId,
    },
}

impl fmt::Display for ParamFreeShapeSupportBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid core param-free shape-support closure: {self:?}"
        )
    }
}

impl std::error::Error for ParamFreeShapeSupportBuildError {}

#[derive(Debug)]
pub enum ParamFreeShapeSupportValidationError {
    Encode(EncodeError),
    Identity(scoop_identity::IdentityReferenceError),
    Expected(ParamFreeShapeSupportBuildError),
    SourceNominalNotAvailable,
    WrongRoot,
    PlanMismatch,
}

impl fmt::Display for ParamFreeShapeSupportValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid decoded core param-free shape-support plan: {self:?}"
        )
    }
}

impl std::error::Error for ParamFreeShapeSupportValidationError {}
