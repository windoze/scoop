//! Canonical route witnesses for public cross-Cone re-exports.

use std::collections::BTreeSet;
use std::fmt;

use scoop_identity::{ConeIdentity, PersistentExportBindingId};
use scoop_wire::{Encoder, WireEncode};

mod lookup;
mod wire;
pub use wire::{
    DecodedCanonicalReexportRoutesV1, DecodedReexportRouteHopV1, DecodedReexportRouteV1,
    ReexportRouteResolutionError, ReexportRouteSetValidationError,
};

/// One public binding followed while resolving a re-export route.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ReexportRouteHopV1 {
    exporter: ConeIdentity,
    binding: PersistentExportBindingId,
}

impl ReexportRouteHopV1 {
    pub const fn new(exporter: ConeIdentity, binding: PersistentExportBindingId) -> Self {
        Self { exporter, binding }
    }

    pub const fn exporter(self) -> ConeIdentity {
        self.exporter
    }

    pub const fn binding(self) -> PersistentExportBindingId {
        self.binding
    }
}

impl WireEncode for ReexportRouteHopV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.exporter.encode(encoder)?;
        encoder.field(2)?;
        self.binding.encode(encoder)
    }
}

/// A non-empty, acyclic route beginning at one direct dependency.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ReexportRouteV1 {
    immediate_provider: ConeIdentity,
    hops: Vec<ReexportRouteHopV1>,
}

impl ReexportRouteV1 {
    pub fn try_new(
        immediate_provider: ConeIdentity,
        hops: Vec<ReexportRouteHopV1>,
    ) -> Result<Self, ReexportRouteBuildError> {
        let Some(first) = hops.first() else {
            return Err(ReexportRouteBuildError::Empty);
        };
        if first.exporter != immediate_provider {
            return Err(ReexportRouteBuildError::ImmediateProviderMismatch {
                immediate_provider,
                first_exporter: first.exporter,
            });
        }

        let mut exporters = BTreeSet::new();
        let mut bindings = BTreeSet::new();
        for hop in &hops {
            if !exporters.insert(hop.exporter) {
                return Err(ReexportRouteBuildError::RepeatedExporter(hop.exporter));
            }
            if !bindings.insert(hop.binding) {
                return Err(ReexportRouteBuildError::RepeatedBinding(hop.binding));
            }
        }

        Ok(Self {
            immediate_provider,
            hops,
        })
    }

    pub const fn immediate_provider(&self) -> ConeIdentity {
        self.immediate_provider
    }

    pub fn hops(&self) -> &[ReexportRouteHopV1] {
        &self.hops
    }

    pub fn terminal(&self) -> ReexportRouteHopV1 {
        *self
            .hops
            .last()
            .expect("a re-export route is structurally non-empty")
    }
}

impl WireEncode for ReexportRouteV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.immediate_provider.encode(encoder)?;
        encoder.field(2)?;
        encoder.array(self.hops.len() as u64)?;
        for hop in &self.hops {
            hop.encode(encoder)?;
        }
        Ok(())
    }
}

/// A non-empty, canonical set of all routes authorizing one re-export.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalReexportRoutesV1 {
    routes: Vec<ReexportRouteV1>,
}

impl CanonicalReexportRoutesV1 {
    pub fn try_new(mut routes: Vec<ReexportRouteV1>) -> Result<Self, ReexportRouteSetBuildError> {
        if routes.is_empty() {
            return Err(ReexportRouteSetBuildError::Empty);
        }
        routes.sort_unstable();
        routes.dedup();
        Ok(Self { routes })
    }

    pub fn routes(&self) -> &[ReexportRouteV1] {
        &self.routes
    }
}

impl WireEncode for CanonicalReexportRoutesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.routes.len() as u64)?;
        for route in &self.routes {
            route.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReexportRouteBuildError {
    Empty,
    ImmediateProviderMismatch {
        immediate_provider: ConeIdentity,
        first_exporter: ConeIdentity,
    },
    RepeatedExporter(ConeIdentity),
    RepeatedBinding(PersistentExportBindingId),
}

impl fmt::Display for ReexportRouteBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("a re-export route must contain at least one hop"),
            Self::ImmediateProviderMismatch {
                immediate_provider,
                first_exporter,
            } => write!(
                formatter,
                "re-export route begins at Cone {first_exporter}, not immediate provider {immediate_provider}"
            ),
            Self::RepeatedExporter(exporter) => {
                write!(formatter, "re-export route repeats Cone {exporter}")
            }
            Self::RepeatedBinding(binding) => {
                write!(formatter, "re-export route repeats binding {binding}")
            }
        }
    }
}

impl std::error::Error for ReexportRouteBuildError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReexportRouteSetBuildError {
    Empty,
}

impl fmt::Display for ReexportRouteSetBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a re-export binding must have at least one route")
    }
}

impl std::error::Error for ReexportRouteSetBuildError {}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        BindingTarget, CanonicalIdentifier, ConeCoordinate, DeclarationScope, DefinitionOwnerChain,
        ExportBindingKey, PackagePath, PersistentExportBindingId, SourceDeclarationKey,
        SourceDeclarationSite,
    };
    use scoop_wire::{DecodeLimits, WireEncode, decode_canonical, encode};

    use super::*;

    #[test]
    fn route_requires_its_direct_provider_and_rejects_cycles() {
        let direct = cone("example:direct:1.0.0");
        let terminal = cone("example:terminal:1.0.0");
        let direct_binding = binding(direct, "direct");
        let terminal_binding = binding(terminal, "terminal");

        assert_eq!(
            ReexportRouteV1::try_new(direct, Vec::new()),
            Err(ReexportRouteBuildError::Empty)
        );
        assert_eq!(
            ReexportRouteV1::try_new(
                direct,
                vec![ReexportRouteHopV1::new(terminal, terminal_binding)],
            ),
            Err(ReexportRouteBuildError::ImmediateProviderMismatch {
                immediate_provider: direct,
                first_exporter: terminal,
            })
        );
        assert_eq!(
            ReexportRouteV1::try_new(
                direct,
                vec![
                    ReexportRouteHopV1::new(direct, direct_binding),
                    ReexportRouteHopV1::new(direct, terminal_binding),
                ],
            ),
            Err(ReexportRouteBuildError::RepeatedExporter(direct))
        );
        assert_eq!(
            ReexportRouteV1::try_new(
                direct,
                vec![
                    ReexportRouteHopV1::new(direct, direct_binding),
                    ReexportRouteHopV1::new(terminal, direct_binding),
                ],
            ),
            Err(ReexportRouteBuildError::RepeatedBinding(direct_binding))
        );
    }

    #[test]
    fn route_set_is_non_empty_sorted_deduplicated_and_has_fixed_wire() {
        assert_eq!(
            CanonicalReexportRoutesV1::try_new(Vec::new()),
            Err(ReexportRouteSetBuildError::Empty)
        );
        let (route_a, route_b) = route_pair();
        let routes = CanonicalReexportRoutesV1::try_new(vec![
            route_b.clone(),
            route_a.clone(),
            route_b.clone(),
        ])
        .unwrap();
        let mut expected = vec![route_a, route_b];
        expected.sort_unstable();
        assert_eq!(routes.routes(), expected);

        assert_eq!(
            hex(&encode(&routes).unwrap()),
            "82a20158209c205a1d1dbd9cbb5211528a6af90a8a33be6ec4725e040c1da4c46fb00e74e50281a20158209c205a1d1dbd9cbb5211528a6af90a8a33be6ec4725e040c1da4c46fb00e74e50258200bc1ff1de84f94a8fa8a02a536466d62994ccae22fc08c607e352d61252dc9eaa2015820f243a0c14d36267059927864f8e9f0faa48e5eff8e33618d1600dfb9300c5e4b0281a2015820f243a0c14d36267059927864f8e9f0faa48e5eff8e33618d1600dfb9300c5e4b0258209bbfa7cea3a60e0ea8557dd370bc6a93d995176d949b26baa3efd19a3e1ab3d7"
        );
    }

    #[test]
    fn decoded_routes_resolve_only_through_typed_authority() {
        let (route_a, route_b) = route_pair();
        let expected = CanonicalReexportRoutesV1::try_new(vec![route_a, route_b]).unwrap();
        let decoded = decode_canonical::<DecodedCanonicalReexportRoutesV1>(
            &encode(&expected).unwrap(),
            DecodeLimits::default(),
        )
        .unwrap();
        let mut authority = route_authority(&expected);

        assert_eq!(decoded.resolve(&mut authority).unwrap(), expected);
    }

    #[test]
    fn decoded_routes_reject_duplicate_and_noncanonical_order() {
        let (route_a, route_b) = route_pair();
        let canonical =
            CanonicalReexportRoutesV1::try_new(vec![route_a.clone(), route_b.clone()]).unwrap();
        let mut authority = route_authority(&canonical);
        let first = canonical.routes()[0].clone();
        let second = canonical.routes()[1].clone();

        let duplicate = decode_canonical::<DecodedCanonicalReexportRoutesV1>(
            &encode(&RouteSequence(vec![first.clone(), first])).unwrap(),
            DecodeLimits::default(),
        )
        .unwrap();
        assert!(matches!(
            duplicate.resolve(&mut authority),
            Err(ReexportRouteSetValidationError::Duplicate { index: 1 })
        ));

        let reversed = decode_canonical::<DecodedCanonicalReexportRoutesV1>(
            &encode(&RouteSequence(vec![second, canonical.routes()[0].clone()])).unwrap(),
            DecodeLimits::default(),
        )
        .unwrap();
        assert!(matches!(
            reversed.resolve(&mut authority),
            Err(ReexportRouteSetValidationError::NonCanonicalOrder { index: 1 })
        ));
    }

    fn route_pair() -> (ReexportRouteV1, ReexportRouteV1) {
        let first = cone("example:first:1.0.0");
        let second = cone("example:second:1.0.0");
        let route_a = ReexportRouteV1::try_new(
            first,
            vec![ReexportRouteHopV1::new(first, binding(first, "first"))],
        )
        .unwrap();
        let route_b = ReexportRouteV1::try_new(
            second,
            vec![ReexportRouteHopV1::new(second, binding(second, "second"))],
        )
        .unwrap();
        (route_a, route_b)
    }

    fn route_authority(
        routes: &CanonicalReexportRoutesV1,
    ) -> scoop_identity::ValidatedIdentityGraph {
        let mut pending = scoop_identity::PendingIdentityValidation::new();
        for route in routes.routes() {
            pending
                .register_authority(route.immediate_provider())
                .unwrap();
            for hop in route.hops() {
                if hop.exporter() != route.immediate_provider() {
                    pending.register_authority(hop.exporter()).unwrap();
                }
                pending.register_authority(hop.binding()).unwrap();
            }
        }
        pending.finish().unwrap()
    }

    struct RouteSequence(Vec<ReexportRouteV1>);

    impl WireEncode for RouteSequence {
        fn encode(
            &self,
            encoder: &mut scoop_wire::Encoder,
        ) -> Result<(), scoop_wire::cbor::EncodeError> {
            encoder.array(self.0.len() as u64)?;
            for route in &self.0 {
                route.encode(encoder)?;
            }
            Ok(())
        }
    }

    fn cone(value: &str) -> ConeIdentity {
        let mut parts = value.split(':');
        let group = parts.next().unwrap();
        let name = parts.next().unwrap();
        let version = parts.next().unwrap();
        assert!(parts.next().is_none());
        ConeCoordinate::new(group, name, version)
            .unwrap()
            .identity()
            .unwrap()
    }

    fn binding(exporter: ConeIdentity, name: &str) -> PersistentExportBindingId {
        let declaration = SourceDeclarationKey::function(
            SourceDeclarationSite::new(
                exporter,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("target").unwrap(),
            0,
            None,
            Vec::new(),
        );
        PersistentExportBindingId::from_key(&ExportBindingKey::new(
            exporter,
            PackagePath::root(),
            CanonicalIdentifier::new(name).unwrap(),
            BindingTarget::function(&declaration).unwrap(),
        ))
        .unwrap()
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
