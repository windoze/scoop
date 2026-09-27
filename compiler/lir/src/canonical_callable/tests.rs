use super::*;

mod invoke;
mod projection;
mod support;
mod wire;

// Format-only fixtures have no executable Module. Their leaves are explicit
// test data; real producers always derive leaves from their final Functions.
pub(crate) fn fixture_definitions(
    foundation: &ConeLirFoundation,
) -> CanonicalCallableLirDefinitionsV1 {
    let definitions = function_bodies(foundation)
        .map(|body| CanonicalCallableLirDefinitionV1::new(body, Digest256::from_array([7; 32])))
        .collect();
    CanonicalCallableLirDefinitionsV1::new(definitions, foundation).unwrap()
}
