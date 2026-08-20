use scoop_hir::{OdrFreeHirFoundation, SourceRecord};
use scoop_identity::ConeCoordinate;
use scoop_lir::ValidatedLirTargetSelection;

use super::*;
use crate::strong_compile_decode::tests::cone_named;
use crate::{
    ConeKind, ConeSourceForm, DecodedCrossConeClosure, InternallyClosedCrossConeHirClosure,
    ProfileValidatedCrossConeHirClosure,
};

#[derive(Clone, Copy)]
pub(super) enum Case {
    Direct,
    Transitive,
    Diamond,
    Unreachable,
    RetainedCopy,
    MissingPoint,
    ForeignParameter,
}

pub(super) struct Fixture {
    artifacts: Vec<Vec<u8>>,
    direct: Vec<ConeIdentity>,
    pub(super) current: ConeIdentity,
    pub(super) foreign: ConeIdentity,
    pub(super) source_index: usize,
}

impl Fixture {
    pub(super) fn new(case: Case) -> Self {
        let core = ConeRecord::new(
            ConeCoordinate::reserved_core(),
            ConeKind::Library,
            ConeSourceForm::Manifest,
        )
        .unwrap();
        let core_bytes = cross_cone_artifact_for(core, vec![], empty_cross_cone_hir_interface());
        let core_dependency = decode(&core_bytes).dependency_record();
        let foreign = cone_named("source-provider");
        let source = SourceIdentity::new(
            foreign.identity(),
            NormalizedSourcePath::new("src/Origin.scoop").unwrap(),
        )
        .unwrap();
        let context = SourceContextKey::File {
            source: source.clone(),
        };
        let record = SourceRecord::from_utf8(source.clone(), "value", [0, 5]).unwrap();
        let context_record =
            CborIdentityRecord::<PersistentSourceContextId, _>::from_key(context.clone()).unwrap();
        let mut foundation = base_hir_foundation();
        foundation.set_sources(vec![record.clone()]).unwrap();
        foundation
            .set_source_contexts(vec![context_record.clone()])
            .unwrap();
        let foreign_bytes = cross_cone_artifact_for_with_hir_foundation(
            foreign.clone(),
            vec![core_dependency.clone()],
            &foundation,
            empty_cross_cone_hir_interface(),
        );
        let foreign_dependency = decode(&foreign_bytes).dependency_record();
        let mut artifacts = vec![core_bytes, foreign_bytes];
        let mut current = default_fixture::fixture(default_fixture::Case::Defined);
        let end = if matches!(case, Case::MissingPoint) {
            1
        } else {
            5
        };
        let origin = ExportDefinitionSourceV1::new(
            DefinitionOrigin::new(source, SourceSpan::new(0, end).unwrap(), &context).unwrap(),
        );
        let source_index =
            origins::replace(&mut current, origin, matches!(case, Case::ForeignParameter));
        let mut current_dependencies = vec![core_dependency.clone()];
        if matches!(case, Case::Transitive | Case::Diamond) {
            let names = if matches!(case, Case::Diamond) {
                &['a', 'b'][..]
            } else {
                &['a'][..]
            };
            for name in names {
                let facade = cross_cone_artifact_for(
                    cone_named(&format!("source-facade-{name}")),
                    vec![core_dependency.clone(), foreign_dependency.clone()],
                    empty_cross_cone_hir_interface(),
                );
                current_dependencies.push(decode(&facade).dependency_record());
                artifacts.push(facade);
            }
        } else if !matches!(case, Case::Unreachable) {
            current_dependencies.push(foreign_dependency);
        }
        let mut direct = vec![ConeIdentity::CORE, current.cone.identity()];
        if matches!(case, Case::Unreachable) {
            // The global request reaches both artifacts, but this artifact only
            // depends on core. Retained metadata must not create a new edge.
            direct.push(foreign.identity());
        }
        if matches!(case, Case::Unreachable | Case::RetainedCopy) {
            let original = OdrFreeHirFoundation::try_new(current.foundation.clone()).unwrap();
            let mut sources = original.source_records().to_vec();
            sources.push(record);
            current.foundation.set_sources(sources).unwrap();
            let mut contexts: Vec<_> = original
                .source_context_records()
                .map(|(_, key)| CborIdentityRecord::from_key(key.clone()).unwrap())
                .collect();
            contexts.push(context_record);
            current.foundation.set_source_contexts(contexts).unwrap();
        }
        let bytes = cross_cone_artifact_for_with_hir_foundation(
            current.cone.clone(),
            current_dependencies,
            &current.foundation,
            encode(&current.interface.index_for_wire().unwrap()).unwrap(),
        );
        artifacts.push(bytes);
        direct.sort_unstable();
        Self {
            artifacts,
            direct,
            current: current.cone.identity(),
            foreign: foreign.identity(),
            source_index,
        }
    }

    pub(super) fn closed(&self) -> InternallyClosedCrossConeHirClosure<'_> {
        self.profiled()
            .validate_identities()
            .unwrap()
            .validate_foundation_structure()
            .unwrap()
            .resolve_hir_interfaces()
            .unwrap()
            .validate_hir_productions()
            .unwrap()
            .validate_internal_hir_closures()
            .unwrap()
    }

    pub(super) fn profiled(&self) -> ProfileValidatedCrossConeHirClosure<'_> {
        DecodedCrossConeClosure::new(
            cone_named("source-request").identity(),
            ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
            self.direct.clone(),
            self.artifacts.iter().map(|bytes| decode(bytes)).collect(),
        )
        .validate_profile_graph()
        .unwrap()
    }

    pub(super) fn direct_front(&self) -> InternallyClosedCrossConeHirFrontSections<'_> {
        assert_eq!(self.artifacts.len(), 3);
        assert_eq!(self.direct.len(), 2);
        let core = decode(&self.artifacts[0])
            .validate_foundation_identities(std::iter::empty())
            .unwrap();
        let foreign = decode(&self.artifacts[1])
            .validate_foundation_identities([&core])
            .unwrap();
        let mut current = decode(&self.artifacts[2]);
        let identities = current
            .validate_foundation_identities([&core, &foreign])
            .unwrap();
        current
            .validate_foundation_structure(identities)
            .unwrap()
            .resolve_hir_interface()
            .unwrap()
            .validate_hir_production()
            .unwrap()
            .validate_internal_hir_closures()
            .unwrap()
    }
}

fn decode(bytes: &[u8]) -> DecodedCrossConeHirFrontSections<'_> {
    open_graph(bytes)
        .decode_cross_cone_hir_front_sections()
        .unwrap()
}
