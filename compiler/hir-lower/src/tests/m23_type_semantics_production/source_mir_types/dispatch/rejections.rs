use super::*;

pub(super) fn check(
    input: &SingleConeStrongMirInput,
    hir: &hir::CrossConeTypeSemanticsProductionV1,
    types: &CanonicalParamFreeMirTypeExportsV1,
    authority: MirDispatchSchemaAuthority<'_>,
) {
    let empty = CanonicalParamFreeMirTypeExportsV1::default();
    assert!(matches!(
        lower_dispatch_schemas(hir, input, &empty, authority, &[]),
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
            &[]
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
            &[]
        ),
        Err(Error::Schema(
            scoop_mir::MirDispatchSchemaError::MissingType { .. }
        ))
    ));

    lower_dispatch_schemas(hir, input, types, authority, &[]).unwrap();
}
