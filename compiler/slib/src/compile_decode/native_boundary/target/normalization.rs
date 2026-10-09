//! Target contracts replayed from native uses and declared representation roots.

use super::*;

pub(super) fn validate(
    artifact: &mut ValidatedGraphArtifact<'_>,
    graph: &scoop_identity::ValidatedIdentityGraph,
    view: &NativeBoundaryFoundationView<'_>,
    types: scoop_abi::AbiReplayTypes<'_>,
    representation_roots: &[PersistentExactTypeId],
) -> Result<(), NativeBoundaryCompileError> {
    let target = artifact.target_selection().target();

    let callable_applications = records_by_id(
        std::iter::once(
            graph
                .records::<PersistentCallableApplicationId, CallableApplicationKey>(
                    IdentityLayer::Hir,
                    &WirePath::root().field(17),
                )
                .map_err(NativeBoundaryCompileError::Identity)?,
        ),
        &WirePath::root().field(17),
    )?;
    let initialization_units = records_by_id(
        std::iter::once(
            graph
                .records::<PersistentInitializationUnitId, InitializationUnitKey>(
                    IdentityLayer::Hir,
                    &WirePath::root().field(21),
                )
                .map_err(NativeBoundaryCompileError::Identity)?,
        ),
        &WirePath::root().field(21),
    )?;
    let callback_registrations = records_by_id(
        std::iter::once(
            graph
                .records::<PersistentCallbackRegistrationId, CallbackRegistrationKey>(
                    IdentityLayer::Hir,
                    &WirePath::root().field(25),
                )
                .map_err(NativeBoundaryCompileError::Identity)?,
        ),
        &WirePath::root().field(25),
    )?;
    let callback_applications = records_by_id(
        std::iter::once(
            graph
                .records::<PersistentCallbackApplicationId, CallbackApplicationKey>(
                    IdentityLayer::Mir,
                    &WirePath::root().field(9),
                )
                .map_err(NativeBoundaryCompileError::Identity)?,
        ),
        &WirePath::root().field(9),
    )?;

    let actual_contracts = index_records(
        view.native_contracts,
        NativeExternalContractRecord::source,
        &WirePath::root().field(14),
    )?;
    let application_records = index_records(
        view.callback_applications,
        scoop_mir::CallbackApplicationRecord::application,
        &WirePath::root().field(10),
    )?;
    let actual_signatures = index_records(
        view.c_abi_signatures,
        CanonicalCAbiSignatureFingerprintRecord::fingerprint,
        &WirePath::root().field(15),
    )?;
    let actual_layouts = index_records(
        view.c_abi_layouts,
        CanonicalCAbiLayoutFingerprintRecord::fingerprint,
        &WirePath::root().field(16),
    )?;
    let actual_requirements = records_by_id(
        std::iter::once(
            graph
                .records::<NativeLinkRequirementId, NativeLinkRequirementKey>(
                    IdentityLayer::Lir,
                    &WirePath::root().field(20),
                )
                .map_err(NativeBoundaryCompileError::Identity)?,
        ),
        &WirePath::root().field(20),
    )?;
    let mut expected_contracts = HashMap::new();
    scoop_wire::allocation::try_reserve_map(
        &mut expected_contracts,
        view.source_contracts.len(),
        &WirePath::root().field(14),
    )
    .map_err(NativeBoundaryCompileError::Resource)?;
    let mut normalizer = NativeBoundaryNormalizer::new(
        target,
        &types.exact,
        &callable_applications,
        &initialization_units,
        &types.definitions,
    );

    for exact in representation_roots {
        normalizer.c_storage(*exact)?;
    }

    for source in view.source_contracts {
        let actual = actual_contracts
            .get(&source.id())
            .ok_or(NativeBoundaryTargetError::NativeContractMismatch)?;
        let expected = normalizer.normalize_external(source, actual)?;
        expected_contracts.insert(expected.source(), expected);
    }
    require_equal_records(
        &expected_contracts,
        &actual_contracts,
        NativeBoundaryTargetError::NativeContractMismatch,
    )?;

    for bridge in view.callback_bridges {
        let application = callback_applications.get(&bridge.application()).ok_or(
            NativeBoundaryTargetError::MissingCallbackApplication {
                application: bridge.application(),
            },
        )?;

        let registration = callback_registrations
            .get(&application.registration())
            .ok_or(NativeBoundaryTargetError::MissingCallbackRegistration {
                registration: application.registration(),
            })?;
        let binders = normalizer.binders(application.context())?;
        let expected = normalizer.c_signature(registration.source_signature(), &binders)?;
        if expected.fingerprint() != bridge.signature() {
            return Err(NativeBoundaryTargetError::CallbackSignatureMismatch {
                application: bridge.application(),
            }
            .into());
        }
        insert_entry(
            &mut normalizer.expected_signatures,
            expected.fingerprint(),
            expected,
            &WirePath::root().field(15),
        )?;

        let expected_managed =
            normalizer.managed_signature(registration.managed_signature(), &binders)?;

        let actual = application_records.get(&bridge.application()).ok_or(
            NativeBoundaryTargetError::MissingCallbackApplication {
                application: bridge.application(),
            },
        )?;
        if actual.managed_signature() != &expected_managed {
            return Err(
                NativeBoundaryTargetError::ManagedCallbackSignatureMismatch {
                    application: bridge.application(),
                }
                .into(),
            );
        }
    }

    require_equal_records(
        &normalizer.expected_signatures,
        &actual_signatures,
        NativeBoundaryTargetError::CAbiSignatureSetMismatch,
    )?;

    require_equal_records(
        &normalizer.expected_layouts,
        &actual_layouts,
        NativeBoundaryTargetError::CAbiLayoutSetMismatch,
    )?;

    require_equal_records(
        &normalizer.expected_requirements,
        &actual_requirements,
        NativeBoundaryTargetError::NativeRequirementSetMismatch,
    )
}
