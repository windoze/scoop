use super::*;

#[derive(Debug)]
pub enum DefaultSourceNestedCallableQueryError {
    Resource(WireError),
    Attachment,
    MissingIdentity(DefaultNestedCallableIdentityV1),
    Standalone,
    Missing {
        ordinal: u64,
    },
    Identity {
        site: DefaultNestedCallableSiteV1,
        expected: DefaultNestedCallableIdentityV1,
        actual: DefaultNestedCallableIdentityV1,
    },
}
impl From<WireError> for DefaultSourceNestedCallableQueryError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl<'a, K> DefaultSourceNestedCallablesV1<'a, K> {
    /// Selects a source occurrence before checking the candidate's typed identity.
    pub fn lookup(
        &self,
        site: DefaultNestedCallableSiteV1,
        identity: DefaultNestedCallableIdentityV1,
    ) -> Result<&DefaultSourceNestedCallableOccurrenceV1<'a>, DefaultSourceNestedCallableQueryError>
    {
        use DefaultSourceNestedCallableQueryError as Error;

        let DefaultNestedCallableSiteV1::Body { ordinal } = site else {
            return Err(Error::Standalone);
        };
        let occurrence = usize::try_from(ordinal)
            .ok()
            .and_then(|index| self.occurrences.get(index))
            .ok_or(Error::Missing { ordinal })?;
        let expected = occurrence.descriptor.identity();
        if identity != expected {
            return Err(Error::Identity {
                site,
                expected,
                actual: identity,
            });
        }
        Ok(occurrence)
    }
}
impl std::fmt::Display for DefaultSourceNestedCallableQueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Attachment => {
                f.write_str("default callable has no nested descriptor at its body attachment")
            }
            Self::MissingIdentity(identity) => {
                write!(
                    f,
                    "default body has no attached nested descriptor {identity:?}"
                )
            }
            Self::Standalone => {
                f.write_str("standalone nested query has no source body occurrence")
            }
            Self::Missing { ordinal } => {
                write!(f, "missing source nested callable occurrence {ordinal}")
            }
            Self::Identity {
                site,
                expected,
                actual,
            } => write!(
                f,
                "source nested occurrence {site:?} expects {expected:?}, found {actual:?}"
            ),
        }
    }
}
impl std::error::Error for DefaultSourceNestedCallableQueryError {}
