use super::*;

pub(super) fn annotations(
    queries: &mut dyn hir::SemanticQueries,
) -> Result<Vec<GenerationAnnotation>, Diagnostic> {
    let Some(owner) = queries
        .annotations()
        .targets
        .iter()
        .find_map(|target| match target.target {
            hir::SourceAnnotationTarget::Nominal(owner) => Some(owner),
            _ => None,
        })
    else {
        return Ok(Vec::new());
    };
    let context = hir::SemanticContext::Nominal(owner);
    let declarations = queries.lookup_export(
        context,
        ConeIdentity::CORE,
        &[CanonicalIdentifier::new("GenerateWith").expect("the core API name is canonical")],
        BindingNamespace::Type,
    )?;
    let Some(BindableEntity::Annotation(annotation)) = declarations.first() else {
        return Ok(Vec::new());
    };
    let mut applications = queries
        .annotations()
        .targets
        .iter()
        .flat_map(|target| {
            let owner = match target.target {
                hir::SourceAnnotationTarget::Nominal(owner) => Some(owner),
                _ => None,
            };
            target
                .annotations
                .iter()
                .filter(|application| application.annotation == *annotation)
                .map(move |application| GenerationAnnotation {
                    owner: owner.expect("GenerateWith's ordinary target is a nominal declaration"),
                    application: application.clone(),
                })
        })
        .collect::<Vec<_>>();
    applications.sort_by_key(|annotation| {
        let origin = queries.source_location(annotation.application.definition_origin);
        (origin.source, origin.span.start, origin.span.end)
    });
    Ok(applications)
}

pub(super) fn parameters<'a>(
    application: &'a hir::SourceAnnotationApplication,
    origin: &hir::SemanticSourceLocation,
) -> Result<(&'a str, &'a [hir::AnnotationElementV1]), Diagnostic> {
    let [
        hir::CanonicalAnnotationValueV1::Scalar(hir::CanonicalConstValueV1::String(generator)),
        hir::CanonicalAnnotationValueV1::Array { elements, .. },
    ] = application.arguments.as_slice()
    else {
        return Err(diagnostic(
            origin,
            "GenerateWith requires String and Array<String> parameters",
        ));
    };
    if generator.trim().is_empty() {
        return Err(diagnostic(
            origin,
            "GenerateWith generator name must not be empty or whitespace",
        ));
    }
    Ok((generator, elements))
}

pub(super) fn interface(
    queries: &mut dyn hir::SemanticQueries,
    owner: hir::NominalOwner,
    element: &hir::AnnotationElementV1,
    existing: &[hir::TypeId],
) -> Result<GeneratorBinding, Vec<Diagnostic>> {
    let origin = hir::SemanticSourceLocation {
        source: element.source.clone(),
        span: Span::new(
            u32::try_from(element.span.start_byte()).expect("current annotation spans fit u32"),
            u32::try_from(element.span.end_byte()).expect("current annotation spans fit u32"),
        ),
    };
    let hir::CanonicalAnnotationValueV1::Scalar(hir::CanonicalConstValueV1::String(text)) =
        &element.value
    else {
        return Err(vec![diagnostic(
            &origin,
            "GenerateWith interface elements must be String values",
        )]);
    };
    let locate = |mut diagnostics: Vec<Diagnostic>| {
        for diagnostic in &mut diagnostics {
            diagnostic.file = 0;
            diagnostic.source = Some(Box::new(origin.source.clone()));
            diagnostic.span = Some(origin.span);
        }
        diagnostics
    };
    let syntax = scoop_parser::parse_type(text).map_err(locate)?;
    let context = hir::SemanticContext::Nominal(owner);
    let ty = queries.resolve_type(context, &syntax).map_err(locate)?;
    let description = queries
        .describe_type(context.into(), ty)
        .map_err(|error| locate(vec![error]))?;
    if !matches!(
        description.structure,
        hir::SemanticTypeStructure::Nominal(hir::SemanticNominalDescription {
            kind: hir::StaticNominalKind::Interface,
            ..
        })
    ) {
        return Err(vec![diagnostic(
            &origin,
            "GenerateWith interface element must name an interface application",
        )]);
    }
    let Some(&interface) = existing.iter().find(|&&interface| {
        queries.type_relation(context, hir::SemanticTypeRelation::Equal, ty, interface)
    }) else {
        return Err(vec![diagnostic(
            &origin,
            "GenerateWith selects an interface application not inherited by this type",
        )]);
    };
    Ok(GeneratorBinding { interface, origin })
}
