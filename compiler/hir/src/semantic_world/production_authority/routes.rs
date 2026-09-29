use scoop_identity::{ConeIdentity, ExportBindingKey, PersistentExportBindingId};

use super::CrossConeHirProductionAuthority;
use crate::{CanonicalPublicExportBindingsV1, PublicExportBindingClosureAuthority};

impl PublicExportBindingClosureAuthority for CrossConeHirProductionAuthority<'_, '_> {
    fn closure_node_count(&self) -> usize {
        self.world.provider_count().saturating_add(1)
    }

    fn binding_key(&self, binding: PersistentExportBindingId) -> Option<&ExportBindingKey> {
        // Foundation binding records retain declaration order.

        if let Some(key) = self.current_foundation.export_binding_key(binding) {
            return Some(key);
        }
        for provider in &self.world.providers {
            let foundation = provider.foundation();

            if let Some(key) = foundation.semantic_world_export_binding_key(binding) {
                return Some(key);
            }
        }
        None
    }

    fn public_bindings(&self, exporter: ConeIdentity) -> Option<&CanonicalPublicExportBindingsV1> {
        if exporter == self.world.current {
            return Some(self.current_bindings);
        }
        for provider in &self.world.providers {
            if provider.identity() == exporter {
                return Some(provider.interface().public_bindings());
            }
        }
        None
    }
}
