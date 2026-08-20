use super::*;

pub(super) fn project(
    input: &ConeMirInput,
    types: &CanonicalParamFreeMirTypeExportsV1,
) -> Result<Vec<ObjectSource>, MirObjectProductionError> {
    let expected = types
        .records()
        .iter()
        .filter(|record| {
            matches!(
                record.representation(),
                MirTypeRepresentationV1::Object { .. }
            )
        })
        .count();
    let mut sources = Vec::new();
    reserve(&mut sources, expected)?;
    let module = input.module();
    for root in input.materialization().initialization_roots() {
        let unit = &module.initialization_units[root.unit()];
        if unit.identity.key().specialization_key().is_some() {
            continue;
        }
        let crate::InitializationUnitKind::LazySingleton {
            value,
            published_root,
        } = unit.kind
        else {
            continue;
        };
        let value = &module.singleton_values[value];
        let global = &module.globals[module.singleton_published_roots[published_root].global];

        let exact = module
            .meta
            .source_exact_types
            .get(&global.ty)
            .ok_or(MirObjectProductionError::MissingSingletonType {
                value: value.identity,
            })?
            .identity_record()
            .id();

        let Some(ty) = types.get(exact) else { continue };
        let MirTypeRepresentationV1::Object { backing } = ty.representation() else {
            return Err(MirObjectProductionError::SourceRepresentation { exact });
        };
        if sources.len() == expected {
            return Err(MirObjectProductionError::IncompleteObjects {
                expected,
                actual: sources.len() + 1,
            });
        }
        sources.push(ObjectSource {
            value: value.identity,
            exact,
            backing: *backing,
            unit: *root,
        });
    }
    if sources.len() != expected {
        return Err(MirObjectProductionError::IncompleteObjects {
            expected,
            actual: sources.len(),
        });
    }
    Ok(sources)
}
