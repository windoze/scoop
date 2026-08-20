use super::*;
use std::collections::HashSet;

impl Lowerer {
    pub(super) fn bind_type_annotations(
        &mut self,
        annotations: &[ast::Annotation],
    ) -> Vec<hir::TypeAnnotation> {
        let mut result = Vec::new();
        let mut seen = HashSet::new();
        for annotation in annotations {
            if let Some(site) = annotation.use_site {
                self.error(
                    site.span,
                    "annotation use-site targets are not allowed in a type position".into(),
                );
                continue;
            }
            if annotation
                .name
                .single_text()
                .is_some_and(crate::annotations::is_core_annotation)
            {
                self.error(
                    annotation.span,
                    format!(
                        "core annotation `@{}` is not allowed in a type position",
                        annotation.name
                    ),
                );
                continue;
            }
            let Some(id) = self.resolve_user_annotation(annotation) else {
                continue;
            };
            if !self.check_annotation_rules(
                id,
                hir::AnnotationTargetKind::Type,
                seen.contains(&id),
                &annotation.name.to_string(),
                annotation.span,
            ) {
                continue;
            }
            if let Some(application) = self.bind_annotation_application(annotation, id) {
                seen.insert(id);
                result.push(hir::TypeAnnotation {
                    annotation: id,
                    arguments: application.arguments,
                    source: self.intrinsic_sources[self.current_file].identity.clone(),
                    span: scoop_identity::SourceSpan::new(
                        annotation.span.start.into(),
                        annotation.span.end.into(),
                    )
                    .expect("parser spans are ordered"),
                });
            }
        }
        result
    }

    pub(crate) fn record_annotation_schema_type(
        &mut self,
        source: &ast::TypeRef,
        ty: AnnotationParameterType<hir::TypeId>,
    ) {
        let key = TypeUseKey::new(self.current_file, source.span);
        let direct = (!source.annotations.is_empty())
            .then(|| (AnnotationContext::capture(self), source.annotations.clone()));
        if let Some(usage) = self.type_annotations.uses.get_mut(&key) {
            usage.ty = ty;
            usage.parameters.clear();
            usage.direct = direct;
        } else if direct.is_some() {
            self.type_annotations.uses.insert(
                key,
                PendingTypeUse {
                    ty,
                    parameters: Vec::new(),
                    direct,
                    children: Vec::new(),
                    alias: None,
                },
            );
        }
    }

    pub(super) fn type_annotation_signature(
        &mut self,
        ty: hir::TypeId,
        parameters: &[hir::TypeParamId],
        span: ast::Span,
    ) -> Option<scoop_identity::SignatureTypeKey> {
        let core_types = match &self.core {
            crate::CoreLoweringAuthority::Defined => hir::HirCoreTypeIdentityAuthority::Defined(
                self.defined_intrinsic_type_core.as_ref()?,
            ),
            crate::CoreLoweringAuthority::Imported(protocols) => {
                hir::HirCoreTypeIdentityAuthority::Imported(protocols.fundamental_types())
            }
        };
        let mapper = crate::persistent_types::SignatureTypeMapper::new(
            self,
            self.nominal_identities.as_ref()?,
            core_types,
        );
        let binders = parameters
            .iter()
            .enumerate()
            .map(|(index, &parameter)| hir::HirSignatureBinder {
                parameter,
                depth: 0,
                index: index as u32,
            })
            .collect::<Vec<_>>();
        match mapper.map(ty, &binders) {
            Ok(ty) => Some(ty),
            Err(error) => {
                self.error(span, format!("invalid annotated type: {error}"));
                None
            }
        }
    }
}
