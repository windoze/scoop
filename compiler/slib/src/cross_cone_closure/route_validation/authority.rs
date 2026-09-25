use scoop_hir::{
    CanonicalPublicExportBindingsV1, CrossConeHirInterfaceSectionV1,
    PublicExportBindingClosureAuthority,
};
use scoop_identity::{
    ConeIdentity, ExportBindingKey, PersistentExportBindingId, ValidatedIdentityGraph,
};
use scoop_wire::{BudgetMeter, WireError, WireErrorKind, WirePath};

use super::{CanonicalCrossConeRouteAuthority, RouteProviderView, index};

impl<'a> CanonicalCrossConeRouteAuthority<'a> {
    pub(crate) fn try_new(
        current: ConeIdentity,
        identities: &'a ValidatedIdentityGraph,
        interface: &CrossConeHirInterfaceSectionV1,
        direct: &'a [ConeIdentity],
        providers: &'a [RouteProviderView<'a>],
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Self, WireError> {
        meter.charge_work(1, path)?;
        meter.check_table_entries(direct.len() as u64, path)?;
        meter.check_table_entries(providers.len() as u64, path)?;
        let closure_node_count = providers
            .len()
            .checked_add(1)
            .ok_or_else(|| WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None))?;
        let binding_keys = index::binding_keys(identities, interface, meter, path)?;
        Ok(Self {
            current,
            identities,
            direct,
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

    fn is_direct_dependency(
        &self,
        provider: ConeIdentity,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<bool, WireError> {
        for candidate in self.direct {
            meter.charge_work(1, path)?;
            if *candidate == provider {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn binding_key(
        &self,
        binding: PersistentExportBindingId,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Option<&ExportBindingKey>, WireError> {
        let mut start = 0;
        let mut end = self.binding_keys.len();
        while start < end {
            meter.charge_work(1, path)?;
            let middle = start + (end - start) / 2;
            match self.binding_keys[middle].0.cmp(&binding) {
                std::cmp::Ordering::Less => start = middle + 1,
                std::cmp::Ordering::Greater => end = middle,
                std::cmp::Ordering::Equal => return Ok(Some(self.binding_keys[middle].1.as_ref())),
            }
        }
        Ok(None)
    }

    fn public_bindings(
        &self,
        exporter: ConeIdentity,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Option<&CanonicalPublicExportBindingsV1>, WireError> {
        for provider in self.providers {
            meter.charge_work(1, path)?;
            if provider.identity == exporter {
                return Ok(Some(provider.bindings));
            }
        }
        Ok(None)
    }
}
