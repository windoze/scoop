use super::*;
use crate::types::{ArrayKind, ResolvedTypeName};
use hir::AnnotationParameterType;

impl Lowerer {
    pub(super) fn resolve_annotation_parameter_type(
        &mut self,
        source: &ast::TypeRef,
    ) -> Option<AnnotationParameterType<hir::TypeId>> {
        if let Some(id) = self.annotation_class_type(source)? {
            self.record_annotation_schema_type(source, AnnotationParameterType::Annotation(id));
            return Some(AnnotationParameterType::Annotation(id));
        }
        let arguments = match &source.kind {
            ast::TypeRefKind::Generic(_, args)
            | ast::TypeRefKind::Qualified {
                arguments: args, ..
            }
            | ast::TypeRefKind::AppliedMember {
                arguments: args, ..
            } => args.as_slice(),
            _ => &[],
        };
        if let [element] = arguments
            && let Some(id) = self.annotation_class_type(element)?
        {
            self.record_annotation_schema_type(element, AnnotationParameterType::Annotation(id));
            // Resolve the container with Array's existing formal parameter.
            // Its identity distinguishes a forwarded element from an alias
            // that discards its argument or substitutes a fixed type.
            let array_type = self.array_type(ArrayKind::Immutable, self.unit);
            let array_owner = self.nominal_application(array_type)?.template;
            let mut parameters = match self.nominal_declaration_parameters(array_owner) {
                Ok(parameters) => parameters,
                Err(error) => {
                    self.error(source.span, error.diagnostic("annotation array"));
                    return None;
                }
            };
            let mut parameter = parameters.pop().expect("Array has one formal parameter");
            parameter.name = "$annotation.element".into();
            while self
                .type_params_in_scope
                .iter()
                .any(|p| p.name == parameter.name)
            {
                parameter.name.push('_');
            }
            let marker = self.intern_type(hir::Type::Param(parameter.id));
            let mut container = source.clone();
            let args = match &mut container.kind {
                ast::TypeRefKind::Generic(_, args)
                | ast::TypeRefKind::Qualified {
                    arguments: args, ..
                }
                | ast::TypeRefKind::AppliedMember {
                    arguments: args, ..
                } => args,
                _ => unreachable!("the source has one generic argument"),
            };
            args[0] = ast::TypeRef {
                annotations: Vec::new(),
                kind: ast::TypeRefKind::Named(ast::Ident {
                    text: parameter.name.clone(),
                    span: element.span,
                }),
                span: element.span,
            };
            self.type_params_in_scope.push(parameter);
            let ty = self.resolve_type_ref(&container);
            self.type_params_in_scope.pop();
            let ty = ty?;
            if self
                .array_type_info(ty)
                .is_some_and(|a| a.kind == ArrayKind::Immutable && a.element == marker)
            {
                let hir::SourceNominalId::GenericTemplate(array) =
                    self.nominal_application(ty)?.template
                else {
                    unreachable!("Array has a generic template")
                };
                let value_type = AnnotationParameterType::AnnotationArray { array, element: id };
                self.record_annotation_schema_type(source, value_type);
                return Some(value_type);
            }
            self.error(
                source.span,
                "annotation arrays require the actual one-dimensional core Array type".into(),
            );
            return None;
        }
        let ty = self.resolve_type_ref(source)?;
        if !self.is_annotation_parameter_type(ty) {
            self.error(source.span, "annotation parameters require a scalar, integral enum, annotation class, or one-dimensional Array of these types".into());
            return None;
        }
        Some(AnnotationParameterType::Value(ty))
    }

    fn annotation_class_type(
        &mut self,
        source: &ast::TypeRef,
    ) -> Option<Option<PersistentAnnotationId>> {
        let path = match &source.kind {
            ast::TypeRefKind::Named(name) => std::slice::from_ref(name),
            ast::TypeRefKind::Qualified { path, arguments } if arguments.is_empty() => {
                path.as_slice()
            }
            _ => return Some(None),
        };
        self.resolve_type_name_path(path, &[])
            .ok()
            .map(|resolved| match resolved {
                Some(ResolvedTypeName::Annotation(id)) => Some(id),
                _ => None,
            })
    }

    pub(super) fn annotation_signature_type(
        &mut self,
        ty: hir::TypeId,
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
            self.nominal_identities
                .as_ref()
                .expect("annotation values follow nominal identities"),
            core_types,
        );
        match mapper.map(ty, &[]) {
            Ok(ty) => Some(ty),
            Err(error) => {
                self.error(span, format!("invalid annotation value type: {error}"));
                None
            }
        }
    }
}
