use super::*;

/// Actual typed receiver context passed to the protected access authority.
#[derive(Clone, Copy, Debug)]
pub enum ProtectedDefaultReferenceReceiverV1<'a> {
    None,
    Member {
        expression: &'a DefaultExpressionV1,
        implicit_this: bool,
        direct_super: bool,
    },
    Metadata(DefaultBodyReferenceMetadataV1<'a>),
}

/// An expression reference always has a use; metadata retains its typed context.
#[derive(Clone, Copy, Debug)]
pub enum DefaultReferenceContextV1<'a> {
    Expression {
        usage: ProtectedDefaultExpressionUseV1,
        receiver: DefaultExpressionReferenceReceiverV1<'a>,
    },
    Metadata(DefaultBodyReferenceMetadataV1<'a>),
}
#[derive(Clone, Copy, Debug)]
pub enum DefaultExpressionReferenceReceiverV1<'a> {
    None,
    Member {
        expression: &'a DefaultExpressionV1,
        implicit_this: bool,
        direct_super: bool,
    },
}
impl<'a> DefaultReferenceContextV1<'a> {
    pub const fn expression_use(self) -> Option<ProtectedDefaultExpressionUseV1> {
        match self {
            Self::Expression { usage, .. } => Some(usage),
            Self::Metadata(_) => None,
        }
    }
    pub const fn receiver(self) -> ProtectedDefaultReferenceReceiverV1<'a> {
        match self {
            Self::Expression { receiver, .. } => match receiver {
                DefaultExpressionReferenceReceiverV1::None => {
                    ProtectedDefaultReferenceReceiverV1::None
                }
                DefaultExpressionReferenceReceiverV1::Member {
                    expression,
                    implicit_this,
                    direct_super,
                } => ProtectedDefaultReferenceReceiverV1::Member {
                    expression,
                    implicit_this,
                    direct_super,
                },
            },
            Self::Metadata(metadata) => ProtectedDefaultReferenceReceiverV1::Metadata(metadata),
        }
    }
}
