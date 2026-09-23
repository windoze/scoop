use super::*;

impl<'b, 's, 'a, 'f> DefaultSourceDomainsV1<'b, 's, 'a, 'f> {
    pub(super) fn value_provider(
        &self,
        target: Target<'_>,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<&'b Declarations<'s, 'a, 'f>, Error> {
        let provider = target
            .source_provider(
                self.current.provider(),
                self.current.foundation.identities,
                meter,
                path,
            )
            .map_err(Error::target)?;
        self.provider(provider, meter, path)
    }
}
