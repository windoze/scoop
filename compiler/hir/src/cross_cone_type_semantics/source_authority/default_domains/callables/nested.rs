use super::*;

pub(super) fn local(
    context: &Context<'_, '_>,
    declaration: CallableTemplateOrigin,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), Error> {
    context
        .declaration
        .nested_callables()
        .require_local_declaration(declaration, meter, path)
        .map_err(query_error)
}

pub(super) fn attached<'r, 's>(
    context: &Context<'r, 's>,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<&'r DefaultSourceNestedCallableOccurrenceV1<'s>, Error> {
    context
        .declaration
        .nested_callables()
        .attached_to(context.occurrence, meter, path)
        .map_err(query_error)
}

fn query_error(error: DefaultSourceNestedCallableQueryError) -> Error {
    use DefaultSourceNestedCallableQueryError as QueryError;
    match error {
        QueryError::Resource(error) => Error::Resource(error),
        QueryError::MissingIdentity(identity) => Error::NestedOccurrence(identity),
        QueryError::Identity { expected, .. } => Error::NestedOccurrence(expected),
        QueryError::Attachment | QueryError::Standalone | QueryError::Missing { .. } => {
            Error::NestedAttachment
        }
    }
}
