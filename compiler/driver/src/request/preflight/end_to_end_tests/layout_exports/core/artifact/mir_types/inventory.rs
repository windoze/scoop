use super::*;
use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, DeclarationScope, DefinitionOwnerChain, ExactTypeKey,
    PackagePath, PendingIdentityValidation, PersistentTypeId, SourceDeclarationKey,
    SourceDeclarationSite, SourceNominalKind,
};

pub(super) fn check(replay: &Replay<'_, '_>) {
    let records = replay.section.types().records();
    for record in records {
        let mut missing = records.to_vec();
        missing.retain(|candidate| candidate.exact() != record.exact());
        let error = replay.reject(missing);
        assert!(
            matches!(error, Error::MissingType(exact) if exact == record.exact())
                || matches!(error, Error::Shape(mir::MirShapeSupportError::MissingType { exact }) if exact == record.exact()),
            "{error:?}"
        );
    }
    for provider in [replay.source.provider(), ConeIdentity::SINGLE_FILE] {
        let extra = extra_type(provider, replay.foundation);
        let exact = extra.exact();
        let mut surplus = records.to_vec();
        surplus.push(extra);
        assert!(matches!(replay.reject(surplus), Error::UnexpectedType(actual) if actual == exact));
    }
    let mut shapes = replay.section.shape_support().records().to_vec();
    let missing = shapes.pop().unwrap().source();
    let shapes = mir::CanonicalMirShapeSupportsV1::try_new(
        replay.source.provider(),
        mir::MirShapeSupportAuthority {
            identities: replay.source.metadata().identities,
            types: replay.section.types(),
        },
        shapes,
    )
    .unwrap();
    assert!(matches!(replay.reject_shapes(records.to_vec(), &shapes),
        Error::Shape(mir::MirShapeSupportError::MissingSource { source }) if source == missing));
    let foreign = mir::CanonicalMirShapeSupportsV1::try_new(
        ConeIdentity::SINGLE_FILE,
        mir::MirShapeSupportAuthority {
            identities: replay.source.metadata().identities,
            types: replay.section.types(),
        },
        vec![],
    )
    .unwrap();
    assert!(
        matches!(replay.reject_shapes(records.to_vec(), &foreign), Error::ShapeProvider { expected, actual }
        if expected == replay.source.provider() && actual == ConeIdentity::SINGLE_FILE)
    );
}

fn extra_type(
    provider: ConeIdentity,
    foundation: &mir::OdrFreeMirFoundation,
) -> mir::ParamFreeMirTypeExportV1 {
    let source: CborIdentityRecord<PersistentTypeId, _> =
        CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
            SourceDeclarationSite::new(
                provider,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("UndeclaredExtra").unwrap(),
            SourceNominalKind::Struct,
            0,
        ))
        .unwrap();
    let exact: CborIdentityRecord<PersistentExactTypeId, _> =
        CborIdentityRecord::from_key(ExactTypeKey::Nominal(source.id())).unwrap();
    let mut hir = hir::CanonicalHirFoundation::empty();
    hir.set_types(vec![source.clone()]).unwrap();
    hir.set_exact_types(vec![exact.clone()]).unwrap();
    let hir: hir::DecodedHirFoundation = decoded(&hir);
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(provider).unwrap();
    hir.register_identities(&mut pending).unwrap();
    hir.resolve_identities(&mut pending).unwrap();
    let graph = pending.finish().unwrap();
    mir::ParamFreeMirTypeExportV1::try_new(
        mir::MirTypeBridgeAuthority {
            identities: &graph,
            foundation,
        },
        exact.id(),
        mir::MirTypeOriginV1::SourceNominal(source.id()),
        mir::MirTypeFactsV1::try_new(
            mir::MirValueKindV1::ZeroSizedValue,
            mir::MirGcKindV1::GcFree,
        )
        .unwrap(),
        mir::MirTypeRepresentationV1::Struct {
            fields: vec![],
            c_layout: mir::MirTypeCLayoutPolicyV1::Ordinary,
            interior_mutable: false,
        },
        mir::MirBaseAndInterfacesV1 {
            base: mir::MirBaseClassV1::None,
            interfaces: vec![],
        },
    )
    .unwrap()
}
