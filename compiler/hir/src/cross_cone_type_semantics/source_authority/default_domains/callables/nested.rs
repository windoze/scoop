use super::*;

pub(super) fn local(
    context: &Context<'_, '_>,
    declaration: CallableTemplateOrigin,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), Error> {
    let occurrences = context.declaration.nested_callables().occurrences();
    meter.charge_work((occurrences.len() as u64).saturating_mul(65), path)?;
    let identity = Nested::LocalFunction(declaration);
    // Membership alone supplies no occurrence-dependent ABI or capture facts.
    if occurrences
        .iter()
        .any(|o| o.descriptor().identity() == identity)
    {
        Ok(())
    } else {
        Err(Error::NestedOccurrence(identity))
    }
}

pub(super) fn attached<'r, 's>(
    context: &Context<'r, 's>,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<&'r DefaultSourceNestedCallableOccurrenceV1<'s>, Error> {
    let descriptor = match context.occurrence.attachment {
        DefaultBodyReferenceAttachmentV1::Expression { expression, .. } => {
            match expression.kind() {
                DefaultExpressionKindV1::Lambda(f) => Descriptor::Lambda(f),
                DefaultExpressionKindV1::AnonymousFunction(f) => Descriptor::AnonymousFunction(f),
                DefaultExpressionKindV1::CallableReference(f) => Descriptor::CallableReference(f),
                _ => return Err(Error::NestedAttachment),
            }
        }
        DefaultBodyReferenceAttachmentV1::Metadata(
            DefaultBodyReferenceMetadataV1::LocalFunction(f),
        ) => Descriptor::LocalFunction(f),
        _ => return Err(Error::NestedAttachment),
    };
    let index = context.declaration.nested_callables();
    meter.charge_work((index.occurrences().len() as u64).saturating_mul(65), path)?;
    let source = index
        .occurrences()
        .iter()
        .find(|o| same_node(o.descriptor(), descriptor))
        .ok_or(Error::NestedOccurrence(descriptor.identity()))?;
    // Select by this body occurrence before checking identity. The same invoke
    // identity may occur at several independently substituted descriptors.
    index
        .lookup(source.site(), descriptor.identity(), meter, path)
        .map_err(|error| match error {
            DefaultSourceNestedCallableQueryError::Resource(error) => Error::Resource(error),
            _ => Error::NestedOccurrence(descriptor.identity()),
        })
}

fn same_node(left: Descriptor<'_>, right: Descriptor<'_>) -> bool {
    match (left, right) {
        (Descriptor::LocalFunction(a), Descriptor::LocalFunction(b)) => std::ptr::eq(a, b),
        (Descriptor::Lambda(a), Descriptor::Lambda(b)) => std::ptr::eq(a, b),
        (Descriptor::AnonymousFunction(a), Descriptor::AnonymousFunction(b)) => std::ptr::eq(a, b),
        (Descriptor::CallableReference(a), Descriptor::CallableReference(b)) => std::ptr::eq(a, b),
        _ => false,
    }
}
