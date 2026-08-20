use super::*;

impl Lowerer {
    pub(crate) fn imported_release_callability(
        &self,
        effects: &hir::CallableSourceEffectsV1,
        bindings: &ImportedTypeBindings,
    ) -> Result<hir::ReleaseCallability, String> {
        use hir::ConditionalReleaseCallability::{NoTransition, Unavailable};
        match effects.release_callability() {
            Unavailable => Ok(Unavailable),
            NoTransition { requirements } => {
                let mut requirements = requirements
                    .iter()
                    .map(|binder| {
                        let ty = bindings
                            .get(&binder.signature())
                            .ok_or("dependency release condition names an absent binder")?;
                        match self.types[*ty] {
                            hir::Type::Param(parameter) => Ok(parameter),
                            _ => Err("dependency release condition is not a parameter"),
                        }
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                requirements.sort_by_key(|parameter| parameter.into_raw());
                requirements.dedup();
                Ok(NoTransition { requirements })
            }
        }
    }
}
