use scoop_wire::{BudgetMeter, WirePath};

use super::{ProtectedDefaultBodyClosureError, collect::Collected};
use crate::{
    DefaultBodyReferenceAttachmentV1, DefaultBodyReferenceMetadataV1,
    DefaultBodyReferenceOccurrenceV1, DefaultBodyReferenceTargetV1, DefaultExpressionKindV1,
    DefaultExpressionV1, OptionalTemplateReceiverV1, ProtectedDefaultExpressionUseV1,
    ProtectedDefaultReceiverUseV1,
};

/// Actual typed receiver context. Metadata deliberately retains its full node
/// instead of claiming that an iterator, assignment or binding has no receiver.
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

pub(super) fn project<'a, E>(
    occurrence: DefaultBodyReferenceOccurrenceV1<'a>,
    template_receiver: &OptionalTemplateReceiverV1,
    collected: &Collected<'_>,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<
    (
        Option<ProtectedDefaultExpressionUseV1>,
        ProtectedDefaultReferenceReceiverV1<'a>,
    ),
    ProtectedDefaultBodyClosureError<E>,
> {
    use DefaultBodyReferenceTargetV1 as Target;
    use DefaultExpressionKindV1 as Kind;
    use ProtectedDefaultReferenceReceiverV1 as Receiver;
    let (index, expression) = match occurrence.attachment {
        DefaultBodyReferenceAttachmentV1::Expression { index, expression } => (index, expression),
        DefaultBodyReferenceAttachmentV1::Metadata(metadata) => {
            return Ok((None, Receiver::Metadata(metadata)));
        }
    };
    meter.charge_work(1, path)?;
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
                    receiver_expression_index: collected
                        .expression_index(expression, meter, path)?,
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
    Ok((
        Some(ProtectedDefaultExpressionUseV1::new(index, use_kind)),
        receiver,
    ))
}
