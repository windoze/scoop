use scoop_identity::{DefinitionOriginSubject, SignatureTypeKey};

use super::super::{body, locals::TemplateLocalProjection};
use super::{DefaultEntityProjector, GenericTemplateProductionError as Error};
use crate::{
    CanonicalBinderUseListV1, DefaultCallableDeclarationV1, DefinitionOrigin,
    ExportDefinitionSourceV1, ExportGenericCallableBodyV1, ExportHir, FunctionGenericity,
    FunctionId, FunctionKind, GenericTemplatePredicatesV1, HirSignatureBinder, TypeParamId,
};

pub(super) fn project(
    entities: &DefaultEntityProjector<'_>,
    function_id: FunctionId,
) -> Result<Option<ExportGenericCallableBodyV1>, Error> {
    let export = entities.export();
    let function = &export.functions[function_id];
    let FunctionKind::User(implementation) = &function.kind else {
        return Ok(None);
    };
    let owner = entities
        .callable_declaration(function_id)
        .map_err(Error::Entity)?;
    let subject = match owner {
        DefaultCallableDeclarationV1::Function(id) => DefinitionOriginSubject::Function(id),
        DefaultCallableDeclarationV1::GenericFunction(id) => {
            DefinitionOriginSubject::GenericFunction(id)
        }
        DefaultCallableDeclarationV1::PropertyAccessor(id) => {
            DefinitionOriginSubject::PropertyAccessor(id)
        }
        DefaultCallableDeclarationV1::Generated(id) => {
            DefinitionOriginSubject::GeneratedCallable(id)
        }
    };
    let definition = export
        .export_definition_origins
        .get(subject)
        .ok_or(Error::MissingOrigin(subject))?
        .origin();
    let origin = local_origin(export, definition, subject)?;
    let signatures = crate::production::signatures::HirInterfaceSignatureProjector::new(export);
    let binders = signatures
        .function_binders(function)
        .map_err(Error::Signature)?;
    let (mut locals, local_table) =
        TemplateLocalProjection::project(entities, &implementation.locals, &binders)
            .map_err(|error| Error::Locals(Box::new(error)))?;
    let capture_types = super::captures::project(entities, function_id, &binders, &mut locals)?;
    let parameters = function
        .params
        .iter()
        .map(|parameter| {
            locals
                .selector(parameter.local)
                .map_err(|error| Error::Body(Box::new(error)))
        })
        .collect::<Result<_, _>>()?;
    let statements = body::project_statements(
        entities,
        &locals,
        &binders,
        origin,
        &implementation.statements,
    )
    .map_err(|error| Error::Body(Box::new(error)))?;
    let result = entities
        .type_key(function.return_ty, &binders)
        .map_err(Error::Entity)?;
    let effects =
        crate::production::callable_interfaces::source_function_effects(export, function, &binders)
            .map_err(Error::Effects)?;
    let type_parameters = binder_uses(
        function
            .type_params()
            .into_iter()
            .map(|parameter| parameter.id),
        &binders,
    )?;
    let (no_gc, pointees) = match &function.genericity {
        FunctionGenericity::Plain => (&[][..], &[][..]),
        FunctionGenericity::Generic { definition, .. } => {
            let declaration = &export.generic_functions[*definition];
            (
                declaration.no_gc_type_params.as_slice(),
                declaration.gc_free_pointee_requirements.as_slice(),
            )
        }
        FunctionGenericity::OwnerParameterizedMethod {
            no_gc_type_params,
            gc_free_pointee_requirements,
            ..
        } => (
            no_gc_type_params.as_slice(),
            gc_free_pointee_requirements.as_slice(),
        ),
        FunctionGenericity::GenericMethod { definition, .. } => {
            let declaration = &export.generic_methods[*definition];
            (
                declaration.no_gc_type_params.as_slice(),
                declaration.gc_free_pointee_requirements.as_slice(),
            )
        }
    };
    let predicates = GenericTemplatePredicatesV1::new(
        binder_uses(no_gc.iter().copied(), &binders)?,
        binder_uses(
            pointees.iter().map(|requirement| requirement.type_param),
            &binders,
        )?,
    );
    ExportGenericCallableBodyV1::try_new(
        owner,
        local_table,
        parameters,
        statements,
        result,
        effects,
        type_parameters,
        predicates,
        ExportDefinitionSourceV1::new(definition.clone()),
        capture_types,
    )
    .map(Some)
    .map_err(Error::Record)
}

pub(in crate::production::default_templates) fn binder_uses(
    parameters: impl IntoIterator<Item = TypeParamId>,
    binders: &[HirSignatureBinder],
) -> Result<CanonicalBinderUseListV1, Error> {
    let arguments = parameters
        .into_iter()
        .map(|parameter| {
            let binder = binders
                .iter()
                .find(|binder| binder.parameter == parameter)
                .ok_or(Error::MissingBinder(parameter))?;
            Ok(SignatureTypeKey::Binder {
                depth: binder.depth,
                index: binder.index,
            })
        })
        .collect::<Result<_, Error>>()?;
    CanonicalBinderUseListV1::try_new(arguments).map_err(Error::Binders)
}

fn local_origin(
    export: &ExportHir,
    origin: &scoop_identity::DefinitionOrigin,
    subject: DefinitionOriginSubject,
) -> Result<DefinitionOrigin, Error> {
    let (file, source) = export
        .source_files
        .iter()
        .enumerate()
        .find(|(_, source)| &source.identity == origin.source())
        .ok_or_else(|| Error::MissingSource(origin.source().clone()))?;
    let (context, _) = export
        .source_contexts
        .iter()
        .find(|(id, _)| export.source_context_identities[*id].id() == origin.context())
        .ok_or(Error::MissingContext(origin.context()))?;
    Ok(DefinitionOrigin {
        provider: source.provider,
        file: u32::try_from(file).map_err(|_| Error::Span(subject))?,
        context,
        span: crate::Span {
            start: u32::try_from(origin.span().start_byte()).map_err(|_| Error::Span(subject))?,
            end: u32::try_from(origin.span().end_byte()).map_err(|_| Error::Span(subject))?,
        },
    })
}
