use super::*;

pub(super) fn validate_callable_signature_metadata(
    module: &Module,
) -> Result<(), MirValidationError> {
    for (index, record) in module.meta.callable_signatures.iter().enumerate() {
        if signature_exact_types(record.signature()).any(|exact| !exact_type_exists(module, exact))
        {
            return invalid(
                index,
                "the callable signature references an unknown exact type",
            );
        }
    }

    let expected = expected_signatures(module)?;
    for (index, (actual, expected)) in module
        .meta
        .callable_signatures
        .iter()
        .zip(expected.iter())
        .enumerate()
    {
        match actual.subject().compare_sort_key(expected.subject()) {
            std::cmp::Ordering::Less => {
                return invalid(
                    index,
                    "the relation contains an unclaimed callable signature",
                );
            }
            std::cmp::Ordering::Greater => {
                return invalid(index, "the relation is missing a callable signature");
            }
            std::cmp::Ordering::Equal if actual.signature() != expected.signature() => {
                return invalid(
                    index,
                    "the relation records a different signature for the callable subject",
                );
            }
            std::cmp::Ordering::Equal => {}
        }
    }

    match module.meta.callable_signatures.len().cmp(&expected.len()) {
        std::cmp::Ordering::Less => invalid(
            module.meta.callable_signatures.len(),
            "the relation is missing a callable signature",
        ),
        std::cmp::Ordering::Greater => invalid(
            expected.len(),
            "the relation contains an unclaimed callable signature",
        ),
        std::cmp::Ordering::Equal => Ok(()),
    }
}

fn expected_signatures(module: &Module) -> Result<MirCallableSignatures, MirValidationError> {
    let mut entries = Vec::new();
    let mut register = |record: &CallableSignatureRecord| entries.push(record.clone());

    for source in module.meta.source_callable_materializations.iter() {
        register(source.signature_record());
    }
    for (_, bridge) in module.callback_bridges.iter() {
        let Some((_, _)) = bridge.local_definition() else {
            continue;
        };
        register(bridge.identity().signature_record());
    }
    for (_, bridge) in module.foreign_callback_bridges.iter() {
        let record = CallableSignatureRecord::new(
            bridge.application_record.managed_adapter(),
            bridge.application_record.managed_signature().clone(),
        );
        register(&record);
    }
    for (_, adapter) in module.meta.closure_adapters.iter() {
        register(adapter.identity().callable_signature_record());
    }
    for (_, adapter) in module.meta.dynamic_closure_adapters.iter() {
        register(adapter.identity().callable_signature_record());
    }
    for bridge in &module.meta.function_bridges {
        register(bridge.identity().signature_record());
    }
    for (_, coroutine) in module.meta.coroutine_functions.iter() {
        if let CoroutineLowering::StateMachine {
            driver_identity, ..
        } = &coroutine.lowering
        {
            register(driver_identity.signature_record());
        }
    }
    for shell in &module.meta.continuation_shells {
        register(shell.identity().success_signature_record());
        register(shell.identity().failure_signature_record());
    }
    for start in &module.meta.coroutine_starts {
        register(start.identity().signature_record());
    }
    for (_, point) in module.meta.coroutine_resume_points.iter() {
        register(point.identity().success().signature_record());
        register(point.identity().failure().signature_record());
    }
    for adjust in &module.meta.boxing_adjusts {
        register(adjust.identity().signature_record());
    }

    MirCallableSignatures::checked(entries).map_err(|error| match error {
        MirCallableSignatureRelationError::DuplicateSubject { index, .. } => error_at(
            index,
            "multiple callable implementations claim the same signature subject",
        ),
    })
}

fn signature_exact_types(
    signature: &scoop_identity::ExactCallableSignature,
) -> impl Iterator<Item = scoop_identity::PersistentExactTypeId> + '_ {
    signature
        .receiver()
        .into_option()
        .into_iter()
        .chain(signature.parameters().iter().copied())
        .chain(std::iter::once(signature.result()))
}

fn exact_type_exists(module: &Module, exact: scoop_identity::PersistentExactTypeId) -> bool {
    module
        .meta
        .source_exact_types
        .get_by_identity(exact)
        .is_some()
        || module
            .meta
            .generated_exact_types
            .get_by_identity(exact)
            .is_some()
}

fn invalid(index: usize, reason: &'static str) -> Result<(), MirValidationError> {
    Err(error_at(index, reason))
}

fn error_at(index: usize, reason: &'static str) -> MirValidationError {
    MirValidationError {
        location: MirValidationLocation::CallableSignature {
            entry: index as u32,
        },
        kind: MirValidationErrorKind::InvalidCallableSignature { reason },
    }
}
