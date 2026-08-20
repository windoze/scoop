use std::collections::HashSet;

use la_arena::{Arena, Idx};

use super::*;

pub(super) fn validate_generated_exact_type_metadata(
    module: &Module,
) -> Result<(), MirValidationError> {
    for (index, entry) in module.meta.generated_exact_types.iter().enumerate() {
        let location = MirValidationLocation::GeneratedExactType {
            entry: index as u32,
        };
        let exists = match entry.location() {
            GeneratedExactTypeLocation::Context(storage) => {
                entry.nominal_record().key()
                    == &scoop_identity::GeneratedNominalKey::TaskContext(storage)
            }
            GeneratedExactTypeLocation::Closure(class) => {
                arena_get(&module.closure_classes, class).is_some()
            }
            GeneratedExactTypeLocation::Class(class) => arena_get(&module.classes, class).is_some(),
            GeneratedExactTypeLocation::Enum(enumeration) => {
                arena_get(&module.enums, enumeration).is_some()
            }
        };
        if !exists {
            return invalid(location, "the generated nominal location does not exist");
        }
        let rebuilt = GeneratedExactTypeIdentity::new(
            entry.location(),
            entry.nominal_record(),
            entry.owner().odr_member_record(),
        )
        .map_err(|_| {
            error(
                location,
                "the generated nominal does not match its physical arena",
            )
        })?;
        if &rebuilt != entry {
            return invalid(
                location,
                "the generated nominal and exact identities are not canonical",
            );
        }
    }

    let mut expected = HashSet::new();
    for entry in module.meta.generated_exact_types.iter() {
        if let GeneratedExactTypeLocation::Context(_) = entry.location() {
            expected.insert(entry.location());
        }
    }
    for environment in &module.meta.closure_environments {
        expect(
            module,
            &mut expected,
            GeneratedExactTypeLocation::Closure(environment.class()),
            environment.identity().generated_type_record(),
            environment.identity().odr_member_record(),
        )?;
    }
    for (_, adapter) in module.meta.closure_adapters.iter() {
        expect(
            module,
            &mut expected,
            GeneratedExactTypeLocation::Closure(adapter.class()),
            adapter.identity().environment_record(),
            Some(adapter.identity().environment_member_record()),
        )?;
    }
    for (_, adapter) in module.meta.dynamic_closure_adapters.iter() {
        expect(
            module,
            &mut expected,
            GeneratedExactTypeLocation::Closure(adapter.class()),
            adapter.identity().environment_record(),
            Some(adapter.identity().environment_member_record()),
        )?;
    }
    for (_, step) in module.meta.coroutine_steps.iter() {
        expect(
            module,
            &mut expected,
            GeneratedExactTypeLocation::Enum(step.enum_id()),
            step.identity().generated_type_record(),
            step.identity().root().member_record(),
        )?;
    }
    for (_, slot) in module.meta.coroutine_slots.iter() {
        expect(
            module,
            &mut expected,
            GeneratedExactTypeLocation::Enum(slot.enum_id()),
            slot.identity().generated_type_record(),
            slot.identity().root().member_record(),
        )?;
    }
    for (_, frame) in module.meta.coroutine_frames.iter() {
        expect(
            module,
            &mut expected,
            GeneratedExactTypeLocation::Class(frame.class()),
            frame.identity().generated_type_record(),
            frame.identity().odr_member_record(),
        )?;
    }
    for (_, point) in module.meta.coroutine_resume_points.iter() {
        expect(
            module,
            &mut expected,
            GeneratedExactTypeLocation::Class(point.adapter()),
            point.identity().generated_type_record(),
            point.identity().odr_member_record(),
        )?;
    }
    for boxed in &module.meta.boxed_types {
        expect(
            module,
            &mut expected,
            GeneratedExactTypeLocation::Class(boxed.class()),
            boxed.identity().generated_type_record(),
            boxed.identity().root().member_record(),
        )?;
    }
    if expected.len() != module.meta.generated_exact_types.len() {
        return invalid(
            next_location(module),
            "the exact-type relation contains an unclaimed generated nominal",
        );
    }
    Ok(())
}

fn expect(
    module: &Module,
    expected: &mut HashSet<GeneratedExactTypeLocation>,
    location: GeneratedExactTypeLocation,
    nominal: &GeneratedNominalRecord,
    odr_member: Option<&GeneratedExactTypeOdrMemberRecord>,
) -> Result<(), MirValidationError> {
    let error_location = next_location(module);
    if !expected.insert(location) {
        return invalid(
            error_location,
            "a generated nominal location is claimed by multiple MIR transforms",
        );
    }
    let Some(entry) = module.meta.generated_exact_types.get(location) else {
        return invalid(
            error_location,
            "a MIR-generated nominal has no exact-type identity",
        );
    };
    let expected = GeneratedExactTypeIdentity::new(location, nominal, odr_member)
        .map_err(|_| error(error_location, "the generated nominal owner is invalid"))?;
    if entry != &expected {
        return invalid(
            error_location,
            "the exact-type relation names a different generated nominal or materialization owner",
        );
    }
    Ok(())
}

fn next_location(module: &Module) -> MirValidationLocation {
    MirValidationLocation::GeneratedExactType {
        entry: module.meta.generated_exact_types.len() as u32,
    }
}

fn invalid(
    location: MirValidationLocation,
    reason: &'static str,
) -> Result<(), MirValidationError> {
    Err(error(location, reason))
}

fn error(location: MirValidationLocation, reason: &'static str) -> MirValidationError {
    MirValidationError {
        location,
        kind: MirValidationErrorKind::InvalidGeneratedExactType { reason },
    }
}

fn arena_get<T>(arena: &Arena<T>, id: Idx<T>) -> Option<&T> {
    arena
        .iter()
        .find_map(|(candidate, value)| (candidate == id).then_some(value))
}
