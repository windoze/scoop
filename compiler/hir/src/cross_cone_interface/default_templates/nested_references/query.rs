use super::*;

#[derive(Debug)]
pub enum DefaultSourceNestedCallableQueryError {
    MissingIdentity(DefaultNestedCallableIdentityV1),
}

impl std::fmt::Display for DefaultSourceNestedCallableQueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingIdentity(identity) => {
                write!(f, "default body has no local declaration {identity:?}")
            }
        }
    }
}
impl std::error::Error for DefaultSourceNestedCallableQueryError {}
