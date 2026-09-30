use std::collections::{HashMap, HashSet};

use la_arena::Idx;
use scoop_identity::GeneratedCallableKey;

use super::{
    ConstructorIdentityTable, HirClassConstructorIdentity, HirConstructorIdentityError,
    HirConstructorIdentityInputs, HirGeneratedConstructorIdentity, HirSourceConstructorIdentity,
    derive_source_constructor_identity,
};
use crate::{
    ClassConstructorId, ClassConstructorIdentityKind, ClassConstructorKind,
    HirSourceNominalIdentity, TypeParamDecl,
};

pub(super) fn validate(
    inputs: &HirConstructorIdentityInputs<'_>,
    structs: &[HirSourceConstructorIdentity],
    classes: &[HirClassConstructorIdentity],
) -> Result<(), HirConstructorIdentityError> {
    require_length(
        ConstructorIdentityTable::Struct,
        inputs.struct_constructors.len(),
        structs.len(),
    )?;
    require_length(
        ConstructorIdentityTable::Class,
        inputs.class_constructors.len(),
        classes.len(),
    )?;

    check_struct_ownership(inputs)?;
    check_class_ownership(inputs)?;
    let object_by_backing = object_backing_classes(inputs)?;
    let mut source_ids = HashSet::new();
    for (id, constructor) in inputs.struct_constructors.iter() {
        let owner = source_nominal(&inputs.type_inputs.nominal_identities[constructor.owner])?;
        validate_source_record(
            inputs,
            ConstructorIdentityTable::Struct,
            raw_index(id),
            &structs[local_index(id)],
            owner,
            &inputs.type_inputs.structs[constructor.owner].type_params,
            &constructor.parameters,
        )?;
        if !source_ids.insert(structs[local_index(id)].id()) {
            return Err(HirConstructorIdentityError::DuplicateSourceIdentity {
                table: ConstructorIdentityTable::Struct,
                constructor: raw_index(id),
            });
        }
    }

    let mut generated_ids = HashSet::new();
    let mut adapted_sources = HashSet::new();
    for (id, constructor) in inputs.class_constructors.iter() {
        match (&constructor.identity_kind, &classes[local_index(id)]) {
            (ClassConstructorIdentityKind::Source, HirClassConstructorIdentity::Source(record)) => {
                let (owner, type_parameters) =
                    class_source_context(inputs, &object_by_backing, constructor.owner)?;
                validate_source_record(
                    inputs,
                    ConstructorIdentityTable::Class,
                    raw_index(id),
                    record,
                    owner,
                    type_parameters,
                    &constructor.parameters,
                )?;
                if !source_ids.insert(record.id()) {
                    return Err(HirConstructorIdentityError::DuplicateSourceIdentity {
                        table: ConstructorIdentityTable::Class,
                        constructor: raw_index(id),
                    });
                }
            }
            (
                ClassConstructorIdentityKind::ZeroArgumentAdapter { source },
                HirClassConstructorIdentity::ZeroArgumentAdapter {
                    source: identity_source,
                    record,
                },
            ) => {
                validate_adapter(inputs, classes, id, *source, *identity_source, record)?;
                if !adapted_sources.insert(*source) {
                    return Err(HirConstructorIdentityError::DuplicateAdapter {
                        source: raw_index(*source),
                    });
                }
                if !generated_ids.insert(record.id()) {
                    return Err(HirConstructorIdentityError::DuplicateGeneratedIdentity {
                        constructor: raw_index(id),
                    });
                }
            }
            _ => {
                return Err(HirConstructorIdentityError::IdentityKind {
                    constructor: raw_index(id),
                });
            }
        }
    }
    Ok(())
}

fn validate_source_record(
    inputs: &HirConstructorIdentityInputs<'_>,
    table: ConstructorIdentityTable,
    constructor: u32,
    record: &HirSourceConstructorIdentity,
    owner: &HirSourceNominalIdentity,
    type_parameters: &[TypeParamDecl],
    parameters: &[crate::ConstructorParameter],
) -> Result<(), HirConstructorIdentityError> {
    let expected =
        derive_source_constructor_identity(inputs.type_inputs, owner, type_parameters, parameters)
            .map_err(|error| HirConstructorIdentityError::SourceDerivation {
                table,
                constructor,
                error,
            })?;
    if record != &expected {
        return Err(HirConstructorIdentityError::SourceKey { table, constructor });
    }
    Ok(())
}

fn validate_adapter(
    inputs: &HirConstructorIdentityInputs<'_>,
    identities: &[HirClassConstructorIdentity],
    adapter: ClassConstructorId,
    source: ClassConstructorId,
    identity_source: ClassConstructorId,
    record: &HirGeneratedConstructorIdentity,
) -> Result<(), HirConstructorIdentityError> {
    if source != identity_source || local_index(source) >= inputs.class_constructors.len() {
        return Err(HirConstructorIdentityError::AdapterSource {
            adapter: raw_index(adapter),
            source: raw_index(source),
        });
    }
    let adapter_value = &inputs.class_constructors[adapter];
    let source_value = &inputs.class_constructors[source];
    if !matches!(
        source_value.identity_kind,
        ClassConstructorIdentityKind::Source
    ) {
        return Err(HirConstructorIdentityError::AdapterSource {
            adapter: raw_index(adapter),
            source: raw_index(source),
        });
    }
    let Some(source_record) = identities[local_index(source)].source_record() else {
        return Err(HirConstructorIdentityError::AdapterSource {
            adapter: raw_index(adapter),
            source: raw_index(source),
        });
    };
    if adapter == source
        || adapter_value.owner != source_value.owner
        || !inputs.type_inputs.classes[adapter_value.owner]
            .type_params
            .is_empty()
        || inputs.type_inputs.nominal_identities[adapter_value.owner]
            .source()
            .is_none()
        || !adapter_value.parameters.is_empty()
        || source_value.parameters.is_empty()
        || adapter_value.safety != source_value.safety
        || adapter_value.origin != source_value.origin
        || adapter_value.span != source_value.span
    {
        return Err(HirConstructorIdentityError::AdapterShape {
            adapter: raw_index(adapter),
        });
    }
    let ClassConstructorKind::Secondary { delegation, body } = &adapter_value.kind else {
        return Err(HirConstructorIdentityError::AdapterShape {
            adapter: raw_index(adapter),
        });
    };
    let crate::ClassSecondaryDelegation::This { target, arguments } = delegation else {
        return Err(HirConstructorIdentityError::AdapterShape {
            adapter: raw_index(adapter),
        });
    };
    if !body.locals.is_empty()
        || !body.statements.is_empty()
        || arguments.args.len() != source_value.parameters.len()
    {
        return Err(HirConstructorIdentityError::AdapterShape {
            adapter: raw_index(adapter),
        });
    }
    if local_index(*target) >= inputs.class_constructor_applications.len() {
        return Err(HirConstructorIdentityError::AdapterTarget {
            adapter: raw_index(adapter),
        });
    }
    let constructor_application = &inputs.class_constructor_applications[*target];
    if constructor_application.constructor != source
        || local_index(constructor_application.owner) >= inputs.type_inputs.class_applications.len()
        || constructor_application.owner
            != inputs.type_inputs.classes[adapter_value.owner].self_application
        || inputs.type_inputs.class_applications[constructor_application.owner].template
            != inputs.type_inputs.nominal_identities[adapter_value.owner].declaration_id()
        || !inputs.type_inputs.class_applications[constructor_application.owner]
            .arguments
            .is_empty()
    {
        return Err(HirConstructorIdentityError::AdapterTarget {
            adapter: raw_index(adapter),
        });
    }
    if record.key()
        != &(GeneratedCallableKey::ZeroArgumentConstructorAdapter {
            constructor: source_record.id(),
        })
    {
        return Err(HirConstructorIdentityError::GeneratedKey {
            adapter: raw_index(adapter),
        });
    }
    Ok(())
}

fn check_struct_ownership(
    inputs: &HirConstructorIdentityInputs<'_>,
) -> Result<(), HirConstructorIdentityError> {
    let mut seen = vec![false; inputs.struct_constructors.len()];
    for (owner, declaration) in inputs.type_inputs.structs.iter() {
        for constructor in &declaration.constructors {
            let index = local_index(*constructor);
            if index >= seen.len()
                || seen[index]
                || inputs.struct_constructors[*constructor].owner != owner
            {
                return Err(HirConstructorIdentityError::Ownership {
                    table: ConstructorIdentityTable::Struct,
                    owner: raw_index(owner),
                    constructor: raw_index(*constructor),
                });
            }
            seen[index] = true;
        }
    }
    require_coverage(ConstructorIdentityTable::Struct, &seen)
}

fn check_class_ownership(
    inputs: &HirConstructorIdentityInputs<'_>,
) -> Result<(), HirConstructorIdentityError> {
    let mut seen = vec![false; inputs.class_constructors.len()];
    for (owner, declaration) in inputs.type_inputs.classes.iter() {
        for constructor in &declaration.constructors {
            let index = local_index(*constructor);
            if index >= seen.len()
                || seen[index]
                || inputs.class_constructors[*constructor].owner != owner
            {
                return Err(HirConstructorIdentityError::Ownership {
                    table: ConstructorIdentityTable::Class,
                    owner: raw_index(owner),
                    constructor: raw_index(*constructor),
                });
            }
            seen[index] = true;
        }
    }
    require_coverage(ConstructorIdentityTable::Class, &seen)
}

fn object_backing_classes(
    inputs: &HirConstructorIdentityInputs<'_>,
) -> Result<HashMap<crate::ClassId, crate::ObjectId>, HirConstructorIdentityError> {
    let mut classes = HashMap::new();
    for (object, declaration) in inputs.type_inputs.objects.iter() {
        if local_index(declaration.backing_class) >= inputs.type_inputs.classes.len()
            || classes.insert(declaration.backing_class, object).is_some()
        {
            return Err(HirConstructorIdentityError::ObjectBackingClass {
                object: raw_index(object),
                class: raw_index(declaration.backing_class),
            });
        }
    }
    Ok(classes)
}

fn class_source_context<'a>(
    inputs: &'a HirConstructorIdentityInputs<'_>,
    object_by_backing: &HashMap<crate::ClassId, crate::ObjectId>,
    owner: crate::ClassId,
) -> Result<(&'a HirSourceNominalIdentity, &'a [TypeParamDecl]), HirConstructorIdentityError> {
    if let Some(object) = object_by_backing.get(&owner) {
        source_nominal(&inputs.type_inputs.nominal_identities[*object])
            .map(|identity| (identity, &[][..]))
    } else {
        source_nominal(&inputs.type_inputs.nominal_identities[owner]).map(|identity| {
            (
                identity,
                inputs.type_inputs.classes[owner].type_params.as_slice(),
            )
        })
    }
}

fn source_nominal(
    identity: &crate::HirNominalIdentity,
) -> Result<&HirSourceNominalIdentity, HirConstructorIdentityError> {
    identity
        .source()
        .ok_or(HirConstructorIdentityError::GeneratedOwner)
}

fn require_length(
    table: ConstructorIdentityTable,
    expected: usize,
    actual: usize,
) -> Result<(), HirConstructorIdentityError> {
    if expected == actual {
        Ok(())
    } else {
        Err(HirConstructorIdentityError::Length {
            table,
            expected,
            actual,
        })
    }
}

fn require_coverage(
    table: ConstructorIdentityTable,
    seen: &[bool],
) -> Result<(), HirConstructorIdentityError> {
    if let Some(constructor) = seen.iter().position(|seen| !seen) {
        Err(HirConstructorIdentityError::Unowned {
            table,
            constructor: constructor as u32,
        })
    } else {
        Ok(())
    }
}

fn local_index<T>(id: Idx<T>) -> usize {
    id.into_raw().into_u32() as usize
}

fn raw_index<T>(id: Idx<T>) -> u32 {
    id.into_raw().into_u32()
}
