use scoop_identity::SemanticIdentitySession;
use scoop_lir::ValidatedLirTargetSelection;

use crate::ArtifactCapabilityProfile;
use crate::strong_compile_decode::tests::{build_artifact_for_profile, cone, required_sections};

#[derive(Debug, Eq, PartialEq)]
struct CompileAttempt {
    result: Result<(), String>,
    session_origins: usize,
    session_entities: usize,
}

#[test]
fn arbitrary_archives_are_panic_free_deterministic_and_failure_atomic() {
    let (hir, mir, lir) = required_sections();
    let canonical = build_artifact_for_profile(
        cone(),
        ArtifactCapabilityProfile::SINGLE_CONE_STRONG,
        hir,
        mir,
        lir,
        false,
    );
    let canonical_attempt = compile_attempt(&canonical);
    assert!(canonical_attempt.result.is_ok());
    assert_eq!(canonical_attempt.session_origins, 1);
    assert!(canonical_attempt.session_entities > 0);

    let mut corpus = arbitrary_byte_corpus();
    corpus.push(canonical.clone());

    let step = canonical.len().div_ceil(512).max(1);
    for index in (0..canonical.len()).step_by(step) {
        for mask in [1, 0x80, 0xff] {
            let mut mutated = canonical.clone();
            mutated[index] ^= mask;
            corpus.push(mutated);
        }
    }

    for bytes in corpus {
        let first = compile_attempt(&bytes);
        let second = compile_attempt(&bytes);
        assert_eq!(
            first,
            second,
            "nondeterministic Compile result for input length {}",
            bytes.len()
        );
        if first.result.is_err() {
            assert_eq!(first.session_origins, 0);
            assert_eq!(first.session_entities, 0);
        }
    }
}

fn compile_attempt(bytes: &[u8]) -> CompileAttempt {
    let selection = ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1;
    let mut session = SemanticIdentitySession::new();
    let result = (|| {
        let envelope = crate::DecodedSlibEnvelope::open(bytes, selection)
            .map_err(|error| error.to_string())?;
        let graph = envelope
            .validate_graph()
            .map_err(|error| error.to_string())?;
        crate::validate_single_cone_strong_compile_artifact(graph, &mut session)
            .map_err(|error| error.to_string())?;
        Ok(())
    })();
    CompileAttempt {
        result,
        session_origins: session.origin_count(),
        session_entities: session.entity_count(),
    }
}

fn arbitrary_byte_corpus() -> Vec<Vec<u8>> {
    let mut corpus = vec![Vec::new(), b"!<arch>\n".to_vec(), b"!<thin>\n".to_vec()];
    let mut state = 0xc621_9d73_4a85_0efb_u64;
    for length in 0..=1_024 {
        let mut bytes = Vec::with_capacity(length);
        for _ in 0..length {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            bytes.push(state as u8);
        }
        corpus.push(bytes);
    }
    corpus
}
