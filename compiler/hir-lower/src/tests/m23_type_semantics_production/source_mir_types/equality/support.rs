use super::*;

pub(super) fn boolean(
    input: &SingleConeStrongMirInput,
    graph: &scoop_identity::ValidatedIdentityGraph,
) -> CanonicalParamFreeMirTypeExportsV1 {
    use scoop_mir::*;
    let boolean = input
        .module()
        .meta
        .source_exact_types
        .get(&Type::Boolean)
        .unwrap();
    let scoop_identity::ExactTypeKey::Nominal(nominal) = *boolean.identity_record().key() else {
        unreachable!()
    };
    CanonicalParamFreeMirTypeExportsV1::try_new(vec![
        ParamFreeMirTypeExportV1::try_new(
            MirTypeBridgeAuthority {
                identities: graph,
                foundation: input.foundation(),
            },
            boolean.identity_record().id(),
            MirTypeOriginV1::SourceNominal(nominal),
            MirTypeFactsV1::try_new(MirValueKindV1::NonZeroValue, MirGcKindV1::GcFree).unwrap(),
            MirTypeRepresentationV1::Intrinsic(MirParamFreeIntrinsicV1::Boolean),
            MirBaseAndInterfacesV1 {
                base: MirBaseClassV1::None,
                interfaces: vec![],
            },
        )
        .unwrap(),
    ])
    .unwrap()
}
