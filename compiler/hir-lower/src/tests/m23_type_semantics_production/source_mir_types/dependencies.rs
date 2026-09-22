use super::*;

// Unit is a language builtin. This dependency fixture retains the actual
// imported identity present in MIR; it does not mint a local source type.
pub(super) fn unit(
    input: &scoop_mir::SingleConeStrongMirInput,
    graph: &scoop_identity::ValidatedIdentityGraph,
) -> CanonicalParamFreeMirTypeExportsV1 {
    use scoop_mir::*;
    let Some(unit) = input.module().meta.source_exact_types.get(&Type::Unit) else {
        return CanonicalParamFreeMirTypeExportsV1::default();
    };
    let scoop_identity::ExactTypeKey::Nominal(nominal) = *unit.identity_record().key() else {
        panic!("Unit is nominal")
    };
    CanonicalParamFreeMirTypeExportsV1::try_new(vec![
        ParamFreeMirTypeExportV1::try_new(
            MirTypeBridgeAuthority {
                identities: graph,
                foundation: input.foundation(),
            },
            unit.identity_record().id(),
            MirTypeOriginV1::SourceNominal(nominal),
            MirTypeFactsV1::try_new(MirValueKindV1::ZeroSizedValue, MirGcKindV1::GcFree).unwrap(),
            MirTypeRepresentationV1::Intrinsic(MirParamFreeIntrinsicV1::Unit),
            MirBaseAndInterfacesV1 {
                base: MirBaseClassV1::None,
                interfaces: vec![],
            },
        )
        .unwrap(),
    ])
    .unwrap()
}
