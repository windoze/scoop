use scoop_wire::WireError;

use super::collect::DefaultReferenceExpressionIndexV1;
use crate::{
    DefaultBodyReferenceAttachmentV1, DefaultBodyReferenceMetadataV1,
    DefaultBodyReferenceOccurrenceV1, DefaultBodyReferenceTargetV1, DefaultExpressionKindV1,
    DefaultExpressionV1, OptionalTemplateReceiverV1, ProtectedDefaultExpressionUseV1,
    ProtectedDefaultReceiverUseV1,
};

mod context;
pub use context::*;

pub(crate) enum DefaultReferenceReceiverError {
    Resource(WireError),
    ReceiverOutsideBody,
}
impl From<WireError> for DefaultReferenceReceiverError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}

pub(crate) fn project_default_reference_context<'a>(
    occurrence: DefaultBodyReferenceOccurrenceV1<'a>,
    template_receiver: &OptionalTemplateReceiverV1,
    expressions: &DefaultReferenceExpressionIndexV1,
) -> Result<DefaultReferenceContextV1<'a>, DefaultReferenceReceiverError> {
    use DefaultBodyReferenceTargetV1 as Target;
    use DefaultExpressionKindV1 as Kind;
    use DefaultExpressionReferenceReceiverV1 as Receiver;
    let (index, expression) = match occurrence.attachment {
        DefaultBodyReferenceAttachmentV1::Expression { index, expression } => (index, expression),
        DefaultBodyReferenceAttachmentV1::Metadata(metadata) => {
            return Ok(DefaultReferenceContextV1::Metadata(metadata));
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
    let (use_kind, receiver) = match member {
        Some((expression, direct_super)) => {
            let implicit_this = matches!(
                (expression.kind(), template_receiver.receiver()),
                (Kind::Local(local), Some(receiver)) if local == receiver.local()
            );
            let use_kind = if implicit_this {
                ProtectedDefaultReceiverUseV1::ImplicitThis
            } else {
                ProtectedDefaultReceiverUseV1::Explicit {
                    receiver_expression_index: expressions
                        .get(expression)
                        .ok_or(DefaultReferenceReceiverError::ReceiverOutsideBody)?,
                }
            };
            (
                use_kind,
                Receiver::Member {
                    expression,
                    implicit_this,
                    direct_super,
                },
            )
        }
        None => (ProtectedDefaultReceiverUseV1::None, Receiver::None),
    };
    Ok(DefaultReferenceContextV1::Expression {
        usage: ProtectedDefaultExpressionUseV1::new(index, use_kind),
        receiver,
    })
}
