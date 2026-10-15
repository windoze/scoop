//! Source call signatures retain their logical argument and result types.

use super::*;
use scoop_slib as slib;

mod any_signatures;
mod candidates;
mod changes;

pub(super) use any_signatures::check_link as check_any_link;

pub(super) fn check(
    input: scoop_mir_lower::MirTypeBridgeExportInputV1<'_>,
    provider: scoop_mir_lower::MirTypeBridgeExportInputV1<'_>,
) {
    let foundation = hir::CanonicalHirFoundation::from_type_semantics_output(input.hir).unwrap();
    let provider_foundation =
        hir::CanonicalHirFoundation::from_type_semantics_output(provider.hir).unwrap();
    let metadata = hir::SharedTypeMetadataV1 {
        provider: input.mir.module().cone,
        identities: input.identities,
        foundation: &foundation,
        public: input.public,
    };
    let dependencies = [hir::SharedTypeMetadataV1 {
        provider: provider.mir.module().cone,
        identities: provider.identities,
        foundation: &provider_foundation,
        public: provider.public,
    }];
    metadata.materialized_type_uses(&dependencies).unwrap();
    for candidate in candidates::mutations(input.public) {
        let error = hir::SharedTypeMetadataV1 {
            public: &candidate.public,
            ..metadata
        }
        .materialized_type_uses(&dependencies)
        .unwrap_err();
        let hir::SharedTypeMetadataError::CallSignature { position, source } = error else {
            panic!("expected actual source signature rejection, got {error:?}");
        };
        assert_eq!(position, candidate.position);
        candidate.check_error(&source);
    }
}
