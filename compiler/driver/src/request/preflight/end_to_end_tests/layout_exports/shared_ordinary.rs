use super::*;
use scoop_slib::{
    SharedOrdinaryLirBridgeDependenciesV1, SharedOrdinaryLirBridgeValidationError as Error,
};

mod corruption;
mod dependencies;
mod layouts;

pub(super) use dependencies::check_dependency_uses;

pub(super) fn check(
    name: &str,
    input: scoop_mir_lower::MirTypeBridgeExportInputV1<'_>,
    foundation: &hir::OdrFreeHirFoundation,
    lir: &lir::SingleConeStrongLirOutput,
    layout: &lir::CrossConeLayoutAbiSectionV1<'_>,
    expected: &lir::CrossConeLirBridgeSectionV1,
) {
    let source = hir::SharedTypeMetadataV1 {
        provider: input.mir.module().cone,
        identities: input.identities,
        foundation,
        public: input.public,
    };
    let replay = |layouts: &lir::CanonicalExactLayoutExportsV1| {
        scoop_slib::replay_shared_ordinary_lir_bridge(
            lir.module().meta.target_profile,
            source,
            input.ordinary,
            layouts,
            SharedOrdinaryLirBridgeDependenciesV1 {
                metadata: &[],
                layouts: &[],
                callables: &[],
            },
            lir.foundation(),
        )
    };
    let actual = replay(layout.layouts()).unwrap_or_else(|error| panic!("{name}: {error}"));
    assert_eq!(&actual, expected);
    let wire: lir::DecodedCrossConeLirBridgeSectionV1 = decoded(expected);
    assert_eq!(&wire.validate_against(actual).unwrap(), expected);
    if name.starts_with("shared-ordinary-") {
        assert!(expected.exports().iter().any(|record| matches!(
            record.declaration(),
            scoop_identity::DependencyCallableDeclarationId::PropertyAccessor(_)
        )));
        corruption::check(expected);
        layouts::check(layout.layouts(), lir.foundation(), replay);

        let absent = PendingIdentityValidation::new().finish().unwrap();
        assert!(matches!(
            scoop_slib::replay_shared_ordinary_lir_bridge(
                lir.module().meta.target_profile,
                hir::SharedTypeMetadataV1 {
                    identities: &absent,
                    ..source
                },
                input.ordinary,
                layout.layouts(),
                SharedOrdinaryLirBridgeDependenciesV1 {
                    metadata: &[],
                    layouts: &[],
                    callables: &[]
                },
                lir.foundation(),
            ),
            Err(Error::SourceAbi(_))
        ));
    }
}
