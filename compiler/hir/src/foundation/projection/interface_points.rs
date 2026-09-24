//! Complete source maps from the one common interface and its actual uses.

use super::*;
use scoop_wire::{BudgetMeter, WirePath};

impl CanonicalHirFoundation {
    pub fn complete_cross_cone_interface_source_points(
        &mut self,
        export: &ExportHir,
        interface: &crate::CrossConeHirInterfaceSectionV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), HirFoundationBuildError> {
        let path = WirePath::root();
        let resource = HirFoundationBuildError::SourcePointResource;
        let mut locations = Vec::new();
        for reference in interface.external_references().records() {
            meter.charge_work(1, &path).map_err(resource)?;
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
                meter.charge_work(2, &path).map_err(resource)?;
                meter
                    .charge_owned_bytes(
                        (2 * std::mem::size_of::<(&SourceIdentity, [u64; 2])>()) as u64,
                        &path,
                    )
                    .map_err(resource)?;
                meter
                    .try_reserve_collection_slots(&mut locations, 2, &path)
                    .map_err(resource)?;
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
        for source in &export.source_files {
            meter
                .charge_work(
                    (source.source.len() as u64)
                        .saturating_mul(4)
                        .saturating_add(self.definition_origins.len() as u64)
                        .saturating_add(definitions.len() as u64)
                        .saturating_add(self.sources.len() as u64)
                        .saturating_add(locations.len() as u64),
                    &path,
                )
                .map_err(resource)?;
            let existing = self
                .sources
                .iter()
                .filter(|record| record.identity() == &source.identity)
                .map(|record| record.points().len() as u64)
                .sum::<u64>();
            let mut count = existing.saturating_add(
                2 * (self
                    .definition_origins
                    .iter()
                    .filter(|record| record.origin().source() == &source.identity)
                    .count() as u64
                    + definitions
                        .iter()
                        .filter(|record| record.origin().source() == &source.identity)
                        .count() as u64
                    + locations
                        .iter()
                        .filter(|(identity, _)| **identity == source.identity)
                        .count() as u64),
            );
            let mut lines = 1_u64;
            let mut width = 0_u64;
            for line in source.source.split('\n') {
                lines = lines.saturating_add(1);
                width = width.max(line.len() as u64);
            }
            if let Some(record) = &source.canonical_record {
                count = count.max(record.points().len() as u64);
                lines = lines.max(record.line_starts().len() as u64);
            }
            let search = u64::from(existing.max(count).max(lines).max(1).ilog2());
            let source_search = u64::from((export.source_files.len() as u64).max(1).ilog2());
            let identity_bytes = source.identity.logical_path().as_str().len() as u64;
            meter
                .charge_work(
                    count.saturating_mul(
                        width
                            .saturating_add(search.saturating_mul(2))
                            .saturating_add(
                                identity_bytes
                                    .saturating_add(32)
                                    .saturating_mul(source_search.saturating_add(1)),
                            )
                            .saturating_add(8),
                    ),
                    &path,
                )
                .map_err(resource)?;
            meter
                .charge_collection_slots(
                    count
                        .saturating_mul(4)
                        .saturating_add(lines.saturating_mul(2)),
                    &path,
                )
                .map_err(resource)?;
            meter
                .charge_owned_bytes(
                    count
                        .saturating_mul(
                            2 * std::mem::size_of::<crate::SourcePointRecord>() as u64 + 16,
                        )
                        .saturating_add(count.saturating_mul(identity_bytes))
                        .saturating_add(lines.saturating_mul(16))
                        .saturating_add(identity_bytes.saturating_mul(4).saturating_add(256)),
                    &path,
                )
                .map_err(resource)?;
        }
        self.set_sources(super::source_points::source_records_with_locations(
            &export.source_files,
            &self.definition_origins,
            definitions,
            &self.sources,
            locations,
        )?)
    }
}
