//! Value access follows source declarations on the target's actual provider.

use scoop_hir::{DefaultSourceValueTargetV1 as Target, ExportDefaultReferenceKindV1 as Kind};

use super::CanonicalCrossConeHirSurfaceAuthority;

mod errors;
pub use errors::CrossConeHirDefaultValueAccessError;
type Error = CrossConeHirDefaultValueAccessError;

impl CanonicalCrossConeHirSurfaceAuthority<'_> {
    pub(crate) fn validate_default_value_access(&mut self) -> Result<(), Error> {
        let templates = self.current_interface.default_templates().records();

        for template in templates {
            let refs = template.references();
            let entries =
                refs.constructors()
                    .iter()
                    .enumerate()
                    .map(|(i, r)| {
                        (
                            Kind::Constructor,
                            i,
                            Target::Constructor(r.target()),
                            r.witness(),
                        )
                    })
                    .chain(
                        refs.globals().iter().enumerate().map(|(i, r)| {
                            (Kind::Global, i, Target::Global(*r.target()), r.witness())
                        }),
                    )
                    .chain(refs.singleton_values().iter().enumerate().map(|(i, r)| {
                        (
                            Kind::Singleton,
                            i,
                            Target::Singleton(*r.target()),
                            r.witness(),
                        )
                    }))
                    .chain(
                        refs.fields()
                            .iter()
                            .enumerate()
                            .map(|(i, r)| (Kind::Field, i, Target::Field(r.target()), r.witness())),
                    );
            for (kind, index, target, witness) in entries {
                let mut validate = || -> Result<(), Error> {
                    let expected = self.source_value_access_domain(target)?;
                    let actual = witness.target_domain();

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
