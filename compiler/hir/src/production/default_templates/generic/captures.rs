use scoop_identity::SignatureTypeKey;

use super::super::locals::TemplateLocalProjection;
use super::{DefaultEntityProjector, GenericTemplateProductionError as Error};
use crate::{DefaultCaptureSourceV1, FunctionId, HirSignatureBinder};

pub(super) fn project(
    entities: &DefaultEntityProjector<'_>,
    function: FunctionId,
    binders: &[HirSignatureBinder],
    locals: &mut TemplateLocalProjection,
) -> Result<Vec<SignatureTypeKey>, Error> {
    let export = entities.export();
    if let Some((_, local)) = export
        .local_functions
        .iter()
        .find(|(_, local)| local.source().is_some_and(|(source, _)| source == function))
    {
        let parameters = &export.functions[function].params;
        if parameters.len() < local.captures.len() {
            return Err(Error::CaptureParameters(function));
        }
        for (capture, parameter) in local.captures.iter().zip(parameters) {
            let selector = locals
                .selector(parameter.local)
                .map_err(|error| Error::Body(Box::new(error)))?;
            locals.bind_capture(capture.binding, DefaultCaptureSourceV1::Local(selector));
        }
        return Ok(Vec::new());
    }
    let captures = export
        .lambdas
        .iter()
        .filter_map(|(_, lambda)| {
            lambda
                .definition
                .source()
                .map(|(function, _)| (function, lambda.captures.as_slice()))
        })
        .chain(
            export
                .anonymous_functions
                .iter()
                .filter_map(|(_, anonymous)| {
                    anonymous
                        .definition
                        .source()
                        .map(|(function, _)| (function, anonymous.captures.as_slice()))
                }),
        )
        .find_map(|(body, captures)| (body == function).then_some(captures));
    let Some(captures) = captures else {
        return Ok(Vec::new());
    };
    let mut types = Vec::with_capacity(captures.len());
    for (index, capture) in captures.iter().enumerate() {
        let index = u32::try_from(index).map_err(|_| Error::TooManyCaptures(function))?;
        locals.bind_capture(
            capture.binding,
            DefaultCaptureSourceV1::EnclosingCapture(index),
        );
        types.push(
            entities
                .type_key(capture.ty, binders)
                .map_err(Error::Entity)?,
        );
    }
    Ok(types)
}
