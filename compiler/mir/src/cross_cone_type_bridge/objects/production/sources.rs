use super::*;

pub(super) fn project(
    input: &SingleConeStrongMirInput,
    types: &CanonicalParamFreeMirTypeExportsV1,
    meter: &mut BudgetMeter,
) -> Result<Vec<ObjectSource>, MirObjectProductionError> {
    let path = WirePath::root();
    meter.charge_work(types.records().len() as u64, &path)?;
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
    reserve(&mut sources, expected, meter)?;
    let module = input.module();
    for root in input.materialization().initialization_roots() {
        meter.charge_nodes(1, &path)?;
        let unit = &module.initialization_units[root.unit()];
        let crate::InitializationUnitKind::LazySingleton {
            value,
            published_root,
        } = unit.kind
        else {
            continue;
        };
        let value = &module.singleton_values[value];
        let global = &module.globals[module.singleton_published_roots[published_root].global];
        meter.charge_work(module.meta.source_exact_types.len() as u64, &path)?;
        let exact = module
            .meta
            .source_exact_types
            .get(&global.ty)
            .ok_or(MirObjectProductionError::MissingSingletonType {
                value: value.identity,
            })?
            .identity_record()
            .id();
        meter.charge_work(
            u64::from(types.records().len().checked_ilog2().unwrap_or(0)) + 1,
            &path,
        )?;
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
