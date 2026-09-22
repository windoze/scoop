use super::*;

pub(super) fn check(
    input: &SingleConeStrongMirInput,
    hir: &hir::CrossConeTypeSemanticsProductionV1,
    types: &CanonicalParamFreeMirTypeExportsV1,
    authority: MirDispatchSchemaAuthority<'_>,
) {
    let empty = CanonicalParamFreeMirTypeExportsV1::default();
    assert!(matches!(
        lower_dispatch_schemas(hir, input, &empty, authority, &[], &mut meter()),
        Err(Error::MissingType(_))
    ));
    let callables = CanonicalMirCallableBindingsV1::try_new(Vec::new()).unwrap();
    assert!(matches!(
        lower_dispatch_schemas(
            hir,
            input,
            types,
            MirDispatchSchemaAuthority {
                callables: &callables,
                ..authority
            },
            &[],
            &mut meter()
        ),
        Err(Error::MissingCallable(_))
    ));
    assert!(matches!(
        lower_dispatch_schemas(
            hir,
            input,
            types,
            MirDispatchSchemaAuthority {
                types: &empty,
                ..authority
            },
            &[],
            &mut meter()
        ),
        Err(Error::Schema(
            scoop_mir::MirDispatchSchemaError::MissingType { .. }
        ))
    ));
    let mut measured = meter();
    lower_dispatch_schemas(hir, input, types, authority, &[], &mut measured).unwrap();
    let usage = measured.usage();
    let mut shared = BudgetMeter::new(DecodeLimits {
        validation_work_units: usage.validation_work_units,
        ..DecodeLimits::default()
    });
    lower_dispatch_schemas(hir, input, types, authority, &[], &mut shared).unwrap();
    assert!(matches!(
        lower_dispatch_schemas(hir, input, types, authority, &[], &mut shared),
        Err(Error::Resource(_))
    ));
    assert!(matches!(
        lower_dispatch_schemas(
            hir,
            input,
            types,
            authority,
            &[],
            &mut BudgetMeter::new(DecodeLimits {
                owned_bytes: 0,
                ..DecodeLimits::default()
            })
        ),
        Err(Error::Resource(_))
    ));
}
