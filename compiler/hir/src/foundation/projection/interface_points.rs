//! Complete source maps from the one common interface and its actual uses.

use super::*;
use scoop_wire::WirePath;

impl CanonicalHirFoundation {
    pub fn complete_cross_cone_interface_source_points(
        &mut self,
        export: &ExportHir,
        interface: &crate::CrossConeHirInterfaceSectionV1,
    ) -> Result<(), HirFoundationBuildError> {
        let path = WirePath::root();
        let resource = HirFoundationBuildError::SourcePointResource;
        let mut locations = Vec::new();
        for reference in interface.external_references().records() {
            let origins = reference
                .call_sites()
                .records()
                .iter()
                .map(|site| site.origin())
                .chain(
                    reference
                        .type_sites()
                        .records()
                        .iter()
                        .filter_map(|site| site.as_expression())
                        .map(|site| site.origin()),
                );
            for origin in origins {
                scoop_wire::allocation::try_reserve(&mut locations, 2, &path).map_err(resource)?;
                let definition = origin.definition();
                let evaluation = origin.evaluation();
                locations.push((
                    definition.source(),
                    [definition.span().start_byte(), definition.span().end_byte()],
                ));
                locations.push((
                    evaluation.source(),
                    [evaluation.span().start_byte(), evaluation.span().end_byte()],
                ));
            }
        }
        let definitions = interface.definition_sources().sources();
        self.set_sources(super::source_points::source_records_with_locations(
            &export.source_files,
            &self.definition_origins,
            definitions,
            &self.sources,
            locations,
        )?)
    }
}
