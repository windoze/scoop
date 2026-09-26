//! A physical reference cannot replace the logical Any identity in a call.

use super::*;
use scoop_identity::{CoreBuiltinNominal, ExactTypeKey, PersistentExactTypeId};

pub(in super::super) fn check_link(
    provider: &slib::AssembledCrossConeLayoutStrongArtifactV1,
    artifact: &slib::AssembledCrossConeLayoutStrongArtifactV1,
    profile: &lir::CBridgeToolchainProfileV1,
) -> String {
    use super::super::lir_dependencies::reader as artifact_reader;
    let source = artifact_reader::open_link(artifact)
        .into_shared_sections()
        .unwrap();
    let current = source.identity();
    let expected = source.semantic_fingerprints().code();
    artifact_reader::read_link(provider, artifact)
        .replay_link_symbol_uses(profile)
        .map(|closure| {
            assert_eq!(closure.dependency_first().len(), 2);
            let (physical, proof) = closure.artifact(current).unwrap();
            assert_eq!(physical.identity(), proof.provider());
            assert_eq!(
                expected,
                slib::FingerprintAvailability::Available(proof.code_fingerprint())
            );
            format!(
                "link-objects={} physical={} code={}\n",
                proof.final_objects().objects().len(),
                physical.lir_physical_imports().records().len(),
                proof.code_fingerprint(),
            )
        })
        .unwrap()
}

#[derive(Clone, Copy, Debug)]
enum Slot {
    Argument(usize),
    Result,
}

pub(in super::super) fn check(
    path: &Path,
    input: scoop_mir_lower::MirTypeBridgeExportInputV1<'_>,
    provider_artifact: &slib::AssembledCrossConeLayoutStrongArtifactV1,
    artifact: &slib::AssembledCrossConeLayoutStrongArtifactV1,
) {
    let any = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
        CoreBuiltinNominal::Any.identity_record().id(),
    ))
    .unwrap();
    let hir::concrete::ConcreteCoreProtocols::Imported(protocols) =
        &input.hir.output().local.module().core_protocols
    else {
        panic!("the consumer imports its source String declaration");
    };
    let string = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
        protocols.fundamental_types().string().persistent(),
    ))
    .unwrap();
    assert_ne!(any, string);
    let mut dump = String::new();
    for (reference_index, reference) in input
        .public
        .external_references()
        .records()
        .iter()
        .enumerate()
    {
        for (call_index, call) in reference.call_sites().records().iter().enumerate() {
            let slots = call
                .arguments()
                .iter()
                .enumerate()
                .filter_map(|(index, value)| (*value == any).then_some(Slot::Argument(index)))
                .chain((call.result() == any).then_some(Slot::Result));
            for slot in slots {
                let mut arguments = call.arguments().to_vec();
                let result = match slot {
                    Slot::Argument(index) => {
                        arguments[index] = string;
                        call.result()
                    }
                    Slot::Result => string,
                };
                let replacement = hir::HirDependencyCallSiteV1::try_new(
                    call.position(),
                    call.origin().clone(),
                    arguments,
                    result,
                    call.witness_indices().to_vec(),
                    call.receiver(),
                )
                .unwrap();
                let candidate =
                    changes::call(input.public, reference_index, call_index, replacement);
                let payload = wire::replace_public(artifact, candidate);
                for link in [false, true] {
                    let closure = reader::open(provider_artifact, artifact, &payload, link);
                    let error = match closure.validate_hir_declarations() {
                        Ok(_) => panic!("same-representation substitution passed (link={link})"),
                        Err(error) => error,
                    };
                    assert_eq!(error.provider, input.mir.module().cone);
                    let slib::CrossConeHirDeclarationValidationError::References(
                        slib::CrossConeHirReferenceSurfaceError::CallSites(error),
                    ) = *error.source
                    else {
                        panic!("unexpected HIR rejection: {error:?}");
                    };
                    let slib::CrossConeHirCallSiteOriginError::Signature { position, source } =
                        *error
                    else {
                        panic!("unexpected call rejection: {error:?}");
                    };
                    assert_eq!(position, call.position());
                    check_error(slot, *source, any, string);
                    dump.push_str(&format!(
                        "link={link} call={} reject {slot:?}\n",
                        position.expression_index
                    ));
                }
            }
        }
    }
    assert!(
        !dump.is_empty(),
        "the fixture must contain actual Any signatures"
    );
    if std::env::var_os("SCOOP_UPDATE_ANY_CALLS").is_some() {
        std::fs::write(path, &dump).unwrap();
    }
    assert_eq!(dump, std::fs::read_to_string(path).unwrap());
}

fn check_error(
    slot: Slot,
    error: hir::HirDependencyCallSignatureError,
    any: PersistentExactTypeId,
    string: PersistentExactTypeId,
) {
    let (expected, actual) = match (slot, error) {
        (
            Slot::Argument(selected),
            hir::HirDependencyCallSignatureError::Argument {
                index,
                expected,
                actual,
            },
        ) => {
            assert_eq!(index, selected);
            (expected, actual)
        }
        (Slot::Result, hir::HirDependencyCallSignatureError::Result { expected, actual }) => {
            (expected, actual)
        }
        (_, error) => panic!("unexpected logical signature rejection: {error:?}"),
    };
    assert_eq!((expected, actual), (any, string));
}
