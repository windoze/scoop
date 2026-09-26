use crate::{
    DefaultBodyReferenceAttachmentV1, DefaultBodyReferenceMetadataV1,
    DefaultBodyReferenceOccurrenceV1, DefaultBodyReferenceTargetV1, DefaultExpressionKindV1,
    DefaultExpressionV1, OptionalTemplateReceiverV1,
};

/// The actual typed body node associated with a declaration reference.
#[derive(Clone, Copy, Debug)]
pub enum DefaultReferenceContextV1<'a> {
    Expression {
        index: u32,
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
impl DefaultReferenceContextV1<'_> {
    pub const fn expression_index(self) -> Option<u32> {
        match self {
            Self::Expression { index, .. } => Some(index),
            Self::Metadata(_) => None,
        }
    }
}

pub(super) fn project_default_reference_context<'a>(
    occurrence: DefaultBodyReferenceOccurrenceV1<'a>,
    template_receiver: &OptionalTemplateReceiverV1,
) -> DefaultReferenceContextV1<'a> {
    use DefaultBodyReferenceTargetV1 as Target;
    use DefaultExpressionKindV1 as Kind;
    use DefaultExpressionReferenceReceiverV1 as Receiver;
    let (index, expression) = match occurrence.attachment {
        DefaultBodyReferenceAttachmentV1::Expression { index, expression } => (index, expression),
        DefaultBodyReferenceAttachmentV1::Metadata(metadata) => {
            return DefaultReferenceContextV1::Metadata(metadata);
        }
    };
    let member = match (occurrence.target, expression.kind()) {
        (Target::Field(_), Kind::FieldAccess { receiver, .. })
        | (Target::Callable(_), Kind::MethodCall { receiver, .. }) => {
            Some((receiver.as_ref(), false))
        }
        (Target::Callable(_), Kind::DirectSuperMethodCall { receiver, .. }) => {
            Some((receiver.as_ref(), true))
        }
        _ => None,
    };
    let receiver = match member {
        Some((expression, direct_super)) => Receiver::Member {
            expression,
            implicit_this: matches!((expression.kind(), template_receiver.receiver()),
                (Kind::Local(local), Some(receiver)) if local == receiver.local()),
            direct_super,
        },
        None => Receiver::None,
    };
    DefaultReferenceContextV1::Expression { index, receiver }
}
