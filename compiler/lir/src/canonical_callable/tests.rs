use super::*;

mod odr;
mod support;
mod wire;

// Format-only fixtures have no executable Module. Their leaves are explicit
// test data; real producers always derive leaves from their final Functions.
pub(crate) fn fixture_definitions(foundation: &ConeLirFoundation) -> CanonicalCallableAbisV1 {
    let definitions = function_bodies(foundation)
        .map(|body| CanonicalCallableAbiV1::new(body, CanonicalCallableAbiOwnerV1::Strong))
        .collect();
    CanonicalCallableAbisV1::new(definitions, foundation).unwrap()
}
