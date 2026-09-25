use scoop_identity::{ConeIdentity, ExportBindingKey, PersistentExportBindingId};
use scoop_wire::{BudgetMeter, WireError, WirePath};

use super::CrossConeHirProductionAuthority;
use crate::{CanonicalPublicExportBindingsV1, PublicExportBindingClosureAuthority};

impl PublicExportBindingClosureAuthority for CrossConeHirProductionAuthority<'_, '_> {
    fn closure_node_count(&self) -> usize {
        self.world.provider_count().saturating_add(1)
    }

    fn is_direct_dependency(
        &self,
        identity: ConeIdentity,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<bool, WireError> {
        for provider in &self.world.providers {
            meter.charge_work(1, path)?;
            if provider.identity() == identity {
                return Ok(provider.is_direct());
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
        // Foundation binding records use source order, so these are bounded
        // linear scans. Charge only each foundation actually queried.
        meter.charge_work(
            self.current_foundation.counts().export_bindings as u64 + 1,
            path,
        )?;
        if let Some(key) = self.current_foundation.export_binding_key(binding) {
            return Ok(Some(key));
        }
        for provider in &self.world.providers {
            let foundation = provider.foundation();
            meter.charge_work(foundation.counts().export_bindings as u64 + 1, path)?;
            if let Some(key) = foundation.semantic_world_export_binding_key(binding) {
                return Ok(Some(key));
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
        meter.charge_work(1, path)?;
        if exporter == self.world.current {
            return Ok(Some(self.current_bindings));
        }
        for provider in &self.world.providers {
            meter.charge_work(1, path)?;
            if provider.identity() == exporter {
                return Ok(Some(provider.interface().public_bindings()));
            }
        }
        Ok(None)
    }
}
