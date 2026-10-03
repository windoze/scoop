use scoop_hir::{
    CanonicalPublicExportBindingsV1, CrossConeHirInterfaceSectionV1,
    PublicExportBindingClosureAuthority,
};
use scoop_identity::{
    ConeIdentity, ExportBindingKey, PersistentExportBindingId, ValidatedIdentityGraph,
};
use scoop_wire::{WireError, WireErrorKind, WirePath};

use super::{CanonicalCrossConeRouteAuthority, RouteProviderView, index};

impl<'a> CanonicalCrossConeRouteAuthority<'a> {
    pub(crate) fn try_new(
        current: ConeIdentity,
        identities: &'a ValidatedIdentityGraph,
        interface: &CrossConeHirInterfaceSectionV1,
        providers: &'a [RouteProviderView<'a>],

        path: &WirePath,
    ) -> Result<Self, WireError> {
        let closure_node_count = providers
            .len()
            .checked_add(1)
            .ok_or_else(|| WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None))?;
        let binding_keys = index::binding_keys(identities, interface, path)?;
        Ok(Self {
            current,
            identities,
            providers,
            closure_node_count,
            binding_keys,
        })
    }
}

impl PublicExportBindingClosureAuthority for CanonicalCrossConeRouteAuthority<'_> {
    fn closure_node_count(&self) -> usize {
        self.closure_node_count
    }

    fn binding_key(&self, binding: PersistentExportBindingId) -> Option<&ExportBindingKey> {
        let mut start = 0;
        let mut end = self.binding_keys.len();
        while start < end {
            let middle = start + (end - start) / 2;
            match self.binding_keys[middle].0.cmp(&binding) {
                std::cmp::Ordering::Less => start = middle + 1,
                std::cmp::Ordering::Greater => end = middle,
                std::cmp::Ordering::Equal => return Some(self.binding_keys[middle].1.as_ref()),
            }
        }
        None
    }

    fn public_bindings(&self, exporter: ConeIdentity) -> Option<&CanonicalPublicExportBindingsV1> {
        for provider in self.providers {
            if provider.identity == exporter {
                return Some(provider.bindings);
            }
        }
        None
    }
}
