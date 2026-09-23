//! Replays every actual callable occurrence against shared provider declarations.
use scoop_hir::{
    CoreBootstrapInterfaceSectionV1, DefaultBodyReferenceOccurrenceV1,
    DefaultCallableReferenceTargetViewV1 as View, DefaultSourceNestedCallablesV1,
    ExportDefaultTemplateKeyV1, ExportDefaultTemplateV1,
};
use scoop_wire::WirePath;

use super::CanonicalCrossConeHirSurfaceAuthority;

mod errors;
mod occurrences;
mod targets;
pub use errors::CrossConeHirDefaultCallableAccessError;
type Error = CrossConeHirDefaultCallableAccessError;
type Nested<'a> = DefaultSourceNestedCallablesV1<'a, ExportDefaultTemplateKeyV1>;

struct Context<'r, 'body> {
    nested: &'r Nested<'body>,
    occurrence: DefaultBodyReferenceOccurrenceV1<'body>,
}

impl CanonicalCrossConeHirSurfaceAuthority<'_> {
    pub(crate) fn validate_default_callable_access(
        &mut self,
        protocols: &CoreBootstrapInterfaceSectionV1,
    ) -> Result<(), Error> {
        let path = WirePath::root().field(7);
        let templates = self.current_interface.default_templates().records();
        self.meter
            .check_table_entries(templates.len() as u64, &path)?;
        for (index, template) in templates.iter().enumerate() {
            let path = path.clone().index(index as u64);
            self.validate_template_callable_access(template, protocols, &path)
                .map_err(|source| Error::Template {
                    key: template.key(),
                    source: Box::new(source),
                })?;
        }
        Ok(())
    }

    fn validate_template_callable_access(
        &mut self,
        template: &ExportDefaultTemplateV1,
        protocols: &CoreBootstrapInterfaceSectionV1,
        path: &WirePath,
    ) -> Result<(), Error> {
        let nested = template.index_nested_callables(self.meter, path)?;
        let occurrences = occurrences::collect(template, self.meter, path)?;
        for occurrence in occurrences {
            let index = occurrence.index;
            let context = Context {
                nested: &nested,
                occurrence: occurrence.body,
            };
            let path = path.clone().field(11).field(1).index(index as u64);
            let mut validate = || -> Result<(), Error> {
                let expected = self.source_callable_access_domain(
                    occurrence.target,
                    &context,
                    protocols,
                    &path,
                )?;
                let actual = template.references().callables()[index]
                    .witness()
                    .target_domain();
                let cost = scoop_wire::encoded_length(&expected)
                    .and_then(|left| {
                        scoop_wire::encoded_length(actual).map(|right| left.saturating_add(right))
                    })
                    .map_err(|error| Error::Encoding(error.to_string()))?;
                self.meter.charge_work(cost, &path)?;
                if actual != &expected {
                    return Err(Error::WitnessDomain);
                }
                Ok(())
            };
            validate().map_err(|source| Error::Occurrence {
                index,
                site: occurrence.body.site,
                expression_index: match occurrence.body.attachment {
                    scoop_hir::DefaultBodyReferenceAttachmentV1::Expression { index, .. } => {
                        Some(index)
                    }
                    scoop_hir::DefaultBodyReferenceAttachmentV1::Metadata(_) => None,
                },
                source: Box::new(source),
            })?;
        }
        Ok(())
    }
}
