use scoop_wire::{WireError, decode_canonical, encode};

use super::{IdentityLayer, IdentityValidationError, PendingIdentityValidation};
use crate::{
    CanonicalIdentifier, CborIdentityRecord, DeclarationScope, DecodedCborIdentityRecord,
    DecodedSourceDeclarationKey, DefinitionOwnerChain, PackagePath, PersistentTypeId,
    SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
};

#[derive(Debug, Eq, PartialEq)]
enum ValidationOutcome {
    Wire(WireError),
    Identity(IdentityValidationError),
    Valid { identities: usize, declared: usize },
}

#[test]
fn mutated_identity_records_are_panic_free_and_deterministic() {
    let canonical = encode(&source_type_record()).unwrap();
    let mut corpus = arbitrary_byte_corpus();
    corpus.push(canonical.clone());
    for index in 0..canonical.len() {
        for mask in [1, 0x80, 0xff] {
            let mut mutated = canonical.clone();
            mutated[index] ^= mask;
            corpus.push(mutated);
        }
    }

    for bytes in corpus {
        let first = validate_record(&bytes);
        let second = validate_record(&bytes);
        assert_eq!(
            first, second,
            "nondeterministic identity result for {bytes:02x?}"
        );
    }
}

fn validate_record(bytes: &[u8]) -> ValidationOutcome {
    let record = match decode_canonical::<
        DecodedCborIdentityRecord<PersistentTypeId, DecodedSourceDeclarationKey>,
    >(bytes)
    {
        Ok(record) => record,
        Err(error) => return ValidationOutcome::Wire(error),
    };

    let mut validation = PendingIdentityValidation::new();
    if let Err(error) = validation.register_authority(crate::ConeIdentity::CORE) {
        return ValidationOutcome::Identity(error);
    }
    if let Err(error) = validation.register(IdentityLayer::Hir, &record) {
        return ValidationOutcome::Identity(error);
    }
    if let Err(error) = validation.resolve(&record) {
        return ValidationOutcome::Identity(error);
    }
    match validation.finish() {
        Ok(graph) => ValidationOutcome::Valid {
            identities: graph.identity_count(),
            declared: graph.declared_identity_count(),
        },
        Err(error) => ValidationOutcome::Identity(error),
    }
}

fn source_type_record() -> CborIdentityRecord<PersistentTypeId, SourceDeclarationKey> {
    let site = SourceDeclarationSite::new(
        crate::ConeIdentity::CORE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
        site,
        CanonicalIdentifier::new("Probe").unwrap(),
        SourceNominalKind::Struct,
        0,
    ))
    .unwrap()
}

fn arbitrary_byte_corpus() -> Vec<Vec<u8>> {
    let mut corpus = Vec::new();
    let mut state = 0x53ac_71e2_09d4_b68f_u64;
    for length in 0..=256 {
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
