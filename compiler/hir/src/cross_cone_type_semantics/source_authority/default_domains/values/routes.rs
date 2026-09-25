use super::*;

impl<'b, 's, 'a, 'f> DefaultSourceDomainsV1<'b, 's, 'a, 'f> {
    pub(super) fn value_provider(
        &self,
        target: Target<'_>,
    ) -> Result<&'b Declarations<'s, 'a, 'f>, Error> {
        let provider = target
            .source_provider(self.current.provider(), self.current.foundation.identities)
            .map_err(Error::target)?;
        self.provider(provider)
    }
}
