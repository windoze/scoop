//! Hash-valid Link-only mutations must fail before object consumption.

use super::lir_dependencies::reader;
use super::*;
use scoop_slib as slib;

mod wire;

#[derive(Clone, Copy, Debug)]
enum Failure {
    Member,
    Unit,
    Duplicate,
    MissingObject,
}

pub(super) fn check(
    core: &slib::AssembledCrossConeLayoutStrongArtifactV1,
    artifact: &slib::AssembledCrossConeLayoutStrongArtifactV1,
) {
    for failure in [
        Failure::Member,
        Failure::Unit,
        Failure::Duplicate,
        Failure::MissingObject,
    ] {
        let bytes = wire::replace(artifact, failure);
        let current = open(&bytes, artifact);
        let provider = current.identity();
        let shared = reader::read_sections(
            reader::open_link(core).into_shared_sections().unwrap(),
            current,
        );
        reject(shared, provider, failure);
    }
    let bytes = wire::replace(core, Failure::Unit);
    let dependency = open(&bytes, core);
    let provider = dependency.identity();
    let shared = reader::read_sections(
        dependency,
        reader::open_link(artifact).into_shared_sections().unwrap(),
    );
    reject(shared, provider, Failure::Unit);
}

fn open<'a>(
    bytes: &'a [u8],
    source: &slib::AssembledCrossConeLayoutStrongArtifactV1,
) -> slib::DecodedCrossConeLayoutCompileSections<'a> {
    DecodedSlibEnvelope::open(bytes, source.target_selection())
        .unwrap()
        .validate_graph()
        .unwrap()
        .decode_cross_cone_layout_link_sections()
        .unwrap()
        .into_shared_sections()
        .unwrap()
}

fn reject(
    shared: slib::LirDependencyGraphReplayedCrossConeLayoutClosure<'_>,
    provider: ConeIdentity,
    failure: Failure,
) {
    let error = shared.with_replayed_physical_imports(|_| ()).unwrap_err();
    assert_eq!(error.provider, provider);
    let slib::SharedLirPhysicalError::LinkMaterializations(error) = *error.source else {
        panic!("expected Link materialization failure: {error:?}");
    };
    use slib::{
        LinkObjectMaterializationValidationError as Closure,
        StrongLinkMaterializationError as Error,
    };
    assert!(
        matches!(
            (failure, error.as_ref()),
            (Failure::Member, Error::Closure(Closure::ProjectionMismatch))
                | (
                    Failure::Unit,
                    Error::Closure(Closure::UnknownScoopLirDefinition(_))
                )
                | (
                    Failure::Duplicate,
                    Error::Closure(Closure::MemberPlan(
                        slib::LinkObjectMemberSetPlanError::DuplicateScoopLirDefinition(_),
                    )),
                )
                | (Failure::MissingObject, Error::MissingObjectMember(_))
        ),
        "wrong error for {failure:?}: {error:?}"
    );
}
