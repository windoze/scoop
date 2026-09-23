//! Value access follows source declarations on the target's actual provider.

use scoop_hir::{DefaultSourceValueTargetV1 as Target, ExportDefaultReferenceKindV1 as Kind};
use scoop_wire::WirePath;

use super::CanonicalCrossConeHirSurfaceAuthority;

mod errors;
pub use errors::CrossConeHirDefaultValueAccessError;
type Error = CrossConeHirDefaultValueAccessError;

impl CanonicalCrossConeHirSurfaceAuthority<'_> {
    pub(crate) fn validate_default_value_access(&mut self) -> Result<(), Error> {
        let path = WirePath::root().field(7);
        let templates = self.current_interface.default_templates().records();
        self.meter
            .check_table_entries(templates.len() as u64, &path)?;
        for (template_index, template) in templates.iter().enumerate() {
            let path = path.clone().index(template_index as u64).field(11);
            self.meter.charge_nodes(1, &path)?;
            let refs = template.references();
            let entries =
                refs.constructors()
                    .iter()
                    .enumerate()
                    .map(|(i, r)| {
                        (
                            Kind::Constructor,
                            2,
                            i,
                            Target::Constructor(r.target()),
                            r.witness(),
                        )
                    })
                    .chain(refs.globals().iter().enumerate().map(|(i, r)| {
                        (Kind::Global, 4, i, Target::Global(*r.target()), r.witness())
                    }))
                    .chain(refs.singleton_values().iter().enumerate().map(|(i, r)| {
                        (
                            Kind::Singleton,
                            5,
                            i,
                            Target::Singleton(*r.target()),
                            r.witness(),
                        )
                    }))
                    .chain(
                        refs.fields().iter().enumerate().map(|(i, r)| {
                            (Kind::Field, 6, i, Target::Field(r.target()), r.witness())
                        }),
                    );
            for (kind, tag, index, target, witness) in entries {
                let path = path.clone().field(tag).index(index as u64);
                let mut validate = || -> Result<(), Error> {
                    let expected = self.source_value_access_domain(target, &path)?;
                    let actual = witness.target_domain();
                    let cost = scoop_wire::encoded_length(&expected)
                        .and_then(|left| {
                            scoop_wire::encoded_length(actual)
                                .map(|right| left.saturating_add(right))
                        })
                        .map_err(|error| Error::Encoding(error.to_string()))?;
                    self.meter.charge_work(cost, &path)?;
                    if actual != &expected {
                        return Err(Error::WitnessDomain);
                    }
                    Ok(())
                };
                validate().map_err(|source| Error::Reference {
                    template: template.key(),
                    kind,
                    index,
                    source: Box::new(source),
                })?;
            }
        }
        Ok(())
    }
}
