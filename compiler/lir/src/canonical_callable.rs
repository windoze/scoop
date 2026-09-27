//! Canonical content leaves for the final LIR function bodies.

use std::fmt;

use scoop_identity::PersistentCallableBodyId;
use scoop_wire::{Digest256, HashError, domain_separated_cbor_hash};

use crate::{ConeLirFoundation, Function, Module};

mod encode;
mod wire;

#[cfg(test)]
pub(crate) mod tests;
pub use wire::DecodedCanonicalCallableLirDefinitionsV1;

/// A content hash, distinct from every persistent entity identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalCallableLirDefinitionV1 {
    body: PersistentCallableBodyId,
    fingerprint: Digest256,
}

impl CanonicalCallableLirDefinitionV1 {
    pub const fn new(body: PersistentCallableBodyId, fingerprint: Digest256) -> Self {
        Self { body, fingerprint }
    }

    pub const fn body(self) -> PersistentCallableBodyId {
        self.body
    }

    pub const fn fingerprint(self) -> Digest256 {
        self.fingerprint
    }
}

/// The exact set of LIR Function leaves, including lowered root and
/// initialization gateways as well as ordinary Strong and ODR bodies.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalCallableLirDefinitionsV1 {
    definitions: Vec<CanonicalCallableLirDefinitionV1>,
}

impl CanonicalCallableLirDefinitionsV1 {
    pub fn from_module(
        module: &Module,
        foundation: &ConeLirFoundation,
    ) -> Result<Self, CanonicalCallableLirError> {
        let definitions = module
            .functions
            .iter()
            .map(|function| {
                canonical_callable_lir_fingerprint(module, function)
                    .map(|fingerprint| {
                        CanonicalCallableLirDefinitionV1::new(
                            function.callable_body.id(),
                            fingerprint,
                        )
                    })
                    .map_err(CanonicalCallableLirError::Hash)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(definitions, foundation)
    }

    pub fn new(
        mut definitions: Vec<CanonicalCallableLirDefinitionV1>,
        foundation: &ConeLirFoundation,
    ) -> Result<Self, CanonicalCallableLirError> {
        definitions.sort_by_key(|definition| definition.body);
        let mut expected = function_bodies(foundation).collect::<Vec<_>>();
        // Foundation records are topological: a root gateway follows main
        // even when its body ID sorts first. Content leaves are ID-ordered.
        expected.sort_unstable();
        let actual = definitions
            .iter()
            .map(|definition| definition.body)
            .collect::<Vec<_>>();
        if actual != expected {
            return Err(CanonicalCallableLirError::BodySet { expected, actual });
        }
        Ok(Self { definitions })
    }

    pub fn definitions(&self) -> &[CanonicalCallableLirDefinitionV1] {
        &self.definitions
    }

    pub fn get(&self, body: PersistentCallableBodyId) -> Option<&CanonicalCallableLirDefinitionV1> {
        self.definitions
            .binary_search_by_key(&body, |definition| definition.body)
            .ok()
            .map(|index| &self.definitions[index])
    }
}

fn function_bodies(
    foundation: &ConeLirFoundation,
) -> impl Iterator<Item = PersistentCallableBodyId> + '_ {
    foundation
        .callable_bodies()
        .iter()
        .map(|record| record.id())
}

/// Computes the content of one actual function without referring to its
/// producer, diagnostic names, local arena numbering or object placement.
pub fn canonical_callable_lir_fingerprint(
    module: &Module,
    function: &Function,
) -> Result<Digest256, HashError> {
    domain_separated_cbor_hash(
        "scoop-lir-definition-v1",
        &encode::CallableProjection::new(module, function)?,
    )
}

#[derive(Debug)]
pub enum CanonicalCallableLirError {
    Hash(HashError),
    BodySet {
        expected: Vec<PersistentCallableBodyId>,
        actual: Vec<PersistentCallableBodyId>,
    },
    UnknownBody([u8; 32]),
    UnsortedBodies,
}

impl fmt::Display for CanonicalCallableLirError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid canonical LIR callable definitions: {self:?}")
    }
}
impl std::error::Error for CanonicalCallableLirError {}
