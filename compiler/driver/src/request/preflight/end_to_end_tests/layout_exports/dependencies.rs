use super::*;
use scoop_identity::{ExactTypeKey, RepresentationRole};

// These compiler-pipeline fixtures replay primitive dependencies from the
// actual installed core artifact. They do not claim a complete layout-profile
// section for that artifact, whose generic declarations retain the ODR gate.
pub(super) fn mir_types(
    input: &mir::SingleConeStrongMirInput,
    graph: &ValidatedIdentityGraph,
) -> mir::CanonicalParamFreeMirTypeExportsV1 {
    let mut records = Vec::new();
    for (ty, representation, value_kind, gc) in [
        (
            mir::Type::Unit,
            mir::MirTypeRepresentationV1::Intrinsic(mir::MirParamFreeIntrinsicV1::Unit),
            mir::MirValueKindV1::ZeroSizedValue,
            mir::MirGcKindV1::GcFree,
        ),
        (
            mir::Type::Boolean,
            mir::MirTypeRepresentationV1::Intrinsic(mir::MirParamFreeIntrinsicV1::Boolean),
            mir::MirValueKindV1::NonZeroValue,
            mir::MirGcKindV1::GcFree,
        ),
        (
            mir::Type::String,
            mir::MirTypeRepresentationV1::Intrinsic(mir::MirParamFreeIntrinsicV1::String),
            mir::MirValueKindV1::Reference,
            mir::MirGcKindV1::ContainsManagedReferences,
        ),
        (
            mir::Type::Any,
            mir::MirTypeRepresentationV1::Class {
                kind: mir::MirClassKindV1::Open,
                declared_fields: vec![],
            },
            mir::MirValueKindV1::Reference,
            mir::MirGcKindV1::ContainsManagedReferences,
        ),
    ] {
        let Some(record) = input.module().meta.source_exact_types.get(&ty) else {
            continue;
        };
        let ExactTypeKey::Nominal(nominal) = *record.identity_record().key() else {
            panic!("a parameter-free intrinsic is nominal")
        };
        records.push(
            mir::ParamFreeMirTypeExportV1::try_new(
                mir::MirTypeBridgeAuthority {
                    identities: graph,
                    foundation: input.foundation(),
                },
                record.identity_record().id(),
                mir::MirTypeOriginV1::SourceNominal(nominal),
                mir::MirTypeFactsV1::try_new(value_kind, gc).unwrap(),
                representation,
                mir::MirBaseAndInterfacesV1 {
                    base: mir::MirBaseClassV1::None,
                    interfaces: vec![],
                },
            )
            .unwrap(),
        );
    }
    mir::CanonicalParamFreeMirTypeExportsV1::try_new(records).unwrap()
}

pub(super) fn layouts(
    types: &mir::CanonicalParamFreeMirTypeExportsV1,
    foundation: &lir::OdrFreeLirFoundation,
    graph: &ValidatedIdentityGraph,
    target: lir::LirTargetProfile,
) -> lir::CanonicalExactLayoutExportsV1 {
    let mut records = Vec::new();
    for ty in types.records() {
        let identity = lir::ExactLayoutIdentityV1::from_foundation(
            target,
            graph.canonical_record(ty.exact()).unwrap(),
            RepresentationRole::ManagedValue,
            foundation,
            &mut meter(),
        )
        .unwrap();
        let record = match ty.representation() {
            mir::MirTypeRepresentationV1::Intrinsic(mir::MirParamFreeIntrinsicV1::Unit) => {
                lir::ExactValueLayoutV1::unit(identity, foundation, &mut meter())
            }
            mir::MirTypeRepresentationV1::Intrinsic(mir::MirParamFreeIntrinsicV1::Boolean) => {
                lir::ExactValueLayoutV1::scalar(
                    identity,
                    lir::ScalarRepresentationKindV1::Boolean,
                    foundation,
                    &mut meter(),
                )
            }
            mir::MirTypeRepresentationV1::Intrinsic(mir::MirParamFreeIntrinsicV1::String)
            | mir::MirTypeRepresentationV1::Class { .. } => {
                lir::ExactValueLayoutV1::qualified_pointer(
                    identity,
                    lir::NichePointerKind::Managed,
                    foundation,
                    &mut meter(),
                )
            }
            _ => panic!("fixture primitive dependency"),
        }
        .unwrap();
        records.push(record.into());
    }
    lir::CanonicalExactLayoutExportsV1::try_new(target, foundation, records, &mut meter()).unwrap()
}
