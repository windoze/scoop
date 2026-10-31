//! Available shared implementations are part of the existing Compile dependency.

use super::*;
use crate::CanonicalOdrMemberDirectoryV1;

impl SemanticFingerprintRecord {
    pub(crate) fn with_odr_implementations(
        self,
        members: &CanonicalOdrMemberDirectoryV1,
    ) -> Result<Self, SemanticFingerprintError> {
        let digest = domain_separated_cbor_hash(
            "scoop-lir-odr-availability-v1",
            &ImplementationInput {
                lir: self.lir(),
                members,
            },
        )
        .map_err(SemanticFingerprintError::Hash)?;
        Ok(Self::from_digests(
            self.hir(),
            self.mir(),
            LirFingerprint::from_array(*digest.as_array()),
            self.code(),
            self.runtime_image(),
        ))
    }
}

struct ImplementationInput<'a> {
    lir: LirFingerprint,
    members: &'a CanonicalOdrMemberDirectoryV1,
}

impl WireEncode for ImplementationInput<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.lir.encode(encoder)?;
        encoder.field(2)?;
        self.members.encode_implementation_abis(encoder)
    }
}
