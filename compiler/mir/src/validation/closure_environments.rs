use std::collections::HashSet;

use la_arena::{Arena, Idx};
use scoop_identity::FieldIdentityKey;

use super::*;

pub(super) fn validate_closure_environment_metadata(
    module: &Module,
) -> Result<(), MirValidationError> {
    let adapter_classes = module
        .meta
        .closure_adapters
        .iter()
        .map(|(_, adapter)| adapter.class())
        .chain(
            module
                .meta
                .dynamic_closure_adapters
                .iter()
                .map(|(_, adapter)| adapter.class()),
        )
        .collect::<HashSet<_>>();
    let mut source_classes = HashSet::new();

    for (index, environment) in module.meta.closure_environments.iter().enumerate() {
        let location = MirValidationLocation::ClosureEnvironment {
            environment: index as u32,
        };
        if adapter_classes.contains(&environment.class()) {
            return invalid(location, "an adapter class is claimed as a source closure");
        }
        if !source_classes.insert(environment.class()) {
            return invalid(location, "a source closure class is claimed more than once");
        }
        let Some(class) = arena_get(&module.closure_classes, environment.class()) else {
            return invalid(location, "the source closure class does not exist");
        };
        if ClosureEnvironment::checked(environment.class(), class, environment.identity().clone())
            .is_none()
        {
            return invalid(
                location,
                "the source closure identity does not cover its physical fields",
            );
        }
        let Some(invoke) = arena_get(&module.closure_invoke_functions, class.invoke) else {
            return invalid(
                location,
                "the source closure invoke relation does not exist",
            );
        };
        if arena_get(&module.functions, invoke.function).is_none() {
            return invalid(
                location,
                "the source closure invoke function does not exist",
            );
        }
        let Some(source) = module
            .meta
            .source_callable_materializations
            .get(invoke.function)
        else {
            return invalid(
                location,
                "the source closure invoke has no callable materialization",
            );
        };
        if source.materialization() != environment.identity().callable() {
            return invalid(
                location,
                "the environment owner is not the closure invoke materialization",
            );
        }

        let identity = environment.identity();
        if identity
            .fields()
            .windows(2)
            .any(|pair| pair[0].value_record().id() >= pair[1].value_record().id())
        {
            return invalid(
                location,
                "physical fields are not strictly ordered by persistent local-value id",
            );
        }
        for field in identity.fields() {
            if field.value_record().key().owner().context() != identity.callable().context() {
                return invalid(
                    location,
                    "a captured value has a different materialization context",
                );
            }
            let expected = match field.source() {
                ClosureFieldSource::Capture { .. } => FieldIdentityKey::closure_capture(
                    identity.generated_type_record().key(),
                    field.value_record().id(),
                ),
                ClosureFieldSource::CallableReferenceReceiver => {
                    FieldIdentityKey::callable_reference_receiver(
                        identity.generated_type_record().key(),
                        field.value_record().id(),
                    )
                }
            }
            .map_err(|_| closure_error(location, "a closure field key has an invalid owner"))?;
            if field.field_record().key() != &expected {
                return invalid(
                    location,
                    "a physical field identity does not match its captured value",
                );
            }
        }
        let rebuilt = ClosureEnvironmentIdentity::new(
            identity.callable(),
            identity.role(),
            identity
                .fields()
                .iter()
                .map(|field| (field.source(), field.value_record().clone()))
                .collect(),
            source
                .odr_member_record()
                .map(|member| member.key().group()),
        )
        .map_err(|_| closure_error(location, "the closure identity bundle is inconsistent"))?;
        if &rebuilt != identity {
            return invalid(location, "the closure identity bundle is not canonical");
        }
    }

    for (class, _) in module.closure_classes.iter() {
        if !adapter_classes.contains(&class) && !source_classes.contains(&class) {
            return invalid(
                next_location(module),
                "a source closure class has no persistent environment identity",
            );
        }
    }
    Ok(())
}

fn next_location(module: &Module) -> MirValidationLocation {
    MirValidationLocation::ClosureEnvironment {
        environment: module.meta.closure_environments.len() as u32,
    }
}

fn invalid<T>(
    location: MirValidationLocation,
    reason: &'static str,
) -> Result<T, MirValidationError> {
    Err(closure_error(location, reason))
}

fn closure_error(location: MirValidationLocation, reason: &'static str) -> MirValidationError {
    MirValidationError {
        location,
        kind: MirValidationErrorKind::InvalidClosureEnvironment { reason },
    }
}

fn arena_get<T>(arena: &Arena<T>, id: Idx<T>) -> Option<&T> {
    ((id.into_raw().into_u32() as usize) < arena.len()).then(|| &arena[id])
}
