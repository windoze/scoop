use super::{CurrentUnitImports, ImportCandidateLayer};
use crate::{Lowerer, namespace::PackageId};

impl CurrentUnitImports {
    pub(super) fn layers_in_source_scope(
        &self,
        lowerer: &Lowerer,
        file: usize,
        package: PackageId,
        name: &str,
    ) -> Vec<ImportCandidateLayer> {
        let mut layers = self.block_layers(lowerer, Some(name));
        layers.extend(self.layers(file, package, name));
        for layer in &mut layers {
            let in_scope = |binding: &super::CurrentUnitBindingId| {
                lowerer
                    .top_level_namespaces
                    .source_in_scope(file, self.binding(*binding).file)
            };
            layer.bindings.retain(in_scope);
            layer.suppressed_values.retain(in_scope);
        }
        layers
    }
}

impl Lowerer {
    pub(crate) fn dependency_in_source_scope(
        &self,
        provider: scoop_identity::ConeIdentity,
    ) -> bool {
        self.intrinsic_sources[self.current_file].role == scoop_identity::SourceRole::Test
            || !self.test_dependencies.contains(&provider)
    }

    pub(crate) fn declaration_source_in_scope(&self, file: usize) -> bool {
        self.top_level_namespaces
            .source_in_scope(self.current_file, file)
    }
}
