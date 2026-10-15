use super::*;
use scoop_identity::PersistentAnnotationId;

mod values;

#[derive(Clone, Copy)]
pub(super) enum AnnotationType {
    Value(concrete::TypeId),
    Annotation(PersistentAnnotationId),
    AnnotationArray(PersistentAnnotationId),
}

struct Parameter {
    name: String,
    ty: AnnotationType,
    default: Option<export::CanonicalAnnotationValueV1>,
}

impl Concretizer<'_> {
    fn annotation_parameters(
        &mut self,
        declaration: export::StaticAnnotationDeclaration<'_>,
    ) -> Vec<Parameter> {
        match declaration {
            export::StaticAnnotationDeclaration::Current(declaration) => declaration
                .parameters
                .iter()
                .map(|parameter| {
                    let ty = match parameter.value_type {
                        export::AnnotationParameterType::Value(ty) => {
                            AnnotationType::Value(self.lower_type(ty, &[]))
                        }
                        export::AnnotationParameterType::Annotation(id) => {
                            AnnotationType::Annotation(id)
                        }
                        export::AnnotationParameterType::AnnotationArray { element, .. } => {
                            AnnotationType::AnnotationArray(element)
                        }
                    };
                    Parameter {
                        name: parameter.name.clone(),
                        ty,
                        default: parameter.default.clone(),
                    }
                })
                .collect(),
            export::StaticAnnotationDeclaration::Dependency { declaration, .. } => declaration
                .parameters
                .iter()
                .map(|parameter| {
                    let ty = match &parameter.value_type {
                        export::AnnotationParameterType::Value(ty) => {
                            AnnotationType::Value(self.metadata_signature_type(ty, &[]))
                        }
                        export::AnnotationParameterType::Annotation(id) => {
                            AnnotationType::Annotation(*id)
                        }
                        export::AnnotationParameterType::AnnotationArray { element, .. } => {
                            AnnotationType::AnnotationArray(*element)
                        }
                    };
                    Parameter {
                        name: parameter.name.as_str().to_owned(),
                        ty,
                        default: parameter.default.clone(),
                    }
                })
                .collect(),
        }
    }

    pub(super) fn metadata_annotation_parameters(
        &mut self,
        declaration: export::StaticAnnotationDeclaration<'_>,
        result: concrete::TypeId,
        origin: export::ConcreteExpressionOrigin,
    ) -> concrete::Expr {
        let record = self.metadata_element(result);
        let fields = self.metadata_fields(record);
        let values = self
            .annotation_parameters(declaration)
            .into_iter()
            .enumerate()
            .map(|(index, parameter)| {
                let id = self.metadata_identity(
                    fields[0],
                    format!("annotation-parameter:{}:{index}", declaration.identity()),
                    origin,
                );
                let name = self.metadata_string(parameter.name, origin);
                let ty = self.metadata_annotation_type(parameter.ty, fields[2], origin);
                let default = parameter.default.map(|value| {
                    self.metadata_annotation_value(
                        &value,
                        self.metadata_optional_element(fields[3]),
                        origin,
                    )
                });
                let default = self.metadata_option(fields[3], default, origin);
                self.metadata_record(record, vec![id, name, ty, default], origin)
            })
            .collect();
        self.metadata_array(result, values, origin)
    }

    pub(super) fn metadata_annotations<'a>(
        &mut self,
        annotations: impl IntoIterator<Item = export::StaticAnnotation<'a>>,
        result: concrete::TypeId,
        origin: export::ConcreteExpressionOrigin,
    ) -> concrete::Expr {
        let record = self.metadata_element(result);
        let values = annotations
            .into_iter()
            .map(|annotation| self.metadata_annotation(annotation, record, origin))
            .collect();
        self.metadata_array(result, values, origin)
    }

    fn metadata_annotation(
        &mut self,
        annotation: export::StaticAnnotation<'_>,
        result: concrete::TypeId,
        origin: export::ConcreteExpressionOrigin,
    ) -> concrete::Expr {
        let fields = self.metadata_fields(result);
        let key = self.metadata_annotation_key(annotation.annotation, fields[0], origin);
        let declaration = self
            .source
            .static_annotation_declaration(annotation.annotation, self.dependencies);
        let argument = self.metadata_element(fields[1]);
        let argument_fields = self.metadata_fields(argument);
        let parameters = self.annotation_parameters(declaration);
        assert_eq!(parameters.len(), annotation.arguments.len());
        let arguments = parameters
            .into_iter()
            .zip(annotation.arguments)
            .enumerate()
            .map(|(index, (parameter, value))| {
                let id = self.metadata_identity(
                    argument_fields[0],
                    format!("annotation-parameter:{}:{index}", annotation.annotation),
                    origin,
                );
                let name = self.metadata_string(parameter.name, origin);
                let ty = self.metadata_annotation_type(parameter.ty, argument_fields[2], origin);
                let value = self.metadata_annotation_value(value, argument_fields[3], origin);
                self.metadata_record(argument, vec![id, name, ty, value], origin)
            })
            .collect();
        let arguments = self.metadata_array(fields[1], arguments, origin);
        self.metadata_record(result, vec![key, arguments], origin)
    }

    fn metadata_annotation_type(
        &mut self,
        ty: AnnotationType,
        result: concrete::TypeId,
        origin: export::ConcreteExpressionOrigin,
    ) -> concrete::Expr {
        if let AnnotationType::AnnotationArray(element) = ty {
            let atom = self.metadata_variant(result, "Array").1[0];
            let element =
                self.metadata_annotation_type(AnnotationType::Annotation(element), atom, origin);
            return self.metadata_case(result, "Array", vec![element], origin);
        }
        if let AnnotationType::Value(ty) = ty
            && let concrete::TypeKind::Class(array) = self.types[ty].kind
            && let concrete::ClassRepresentation::Intrinsic {
                application: concrete::IntrinsicTypeRepresentation::Array { element },
                ..
            } = self.classes[array].representation
        {
            let atom = self.metadata_variant(result, "Array").1[0];
            let element =
                self.metadata_annotation_type(AnnotationType::Value(element), atom, origin);
            return self.metadata_case(result, "Array", vec![element], origin);
        }
        let (name, payload) = match ty {
            AnnotationType::Annotation(id) => {
                let key = self.metadata_variant(result, "Annotation").1[0];
                ("Annotation", self.metadata_annotation_key(id, key, origin))
            }
            AnnotationType::Value(ty) => {
                let name = if matches!(self.types[ty].kind, concrete::TypeKind::Enum(_)) {
                    "IntegerEnum"
                } else {
                    "Scalar"
                };
                let reference = self.metadata_variant(result, name).1[0];
                (name, self.type_reference_value(ty, reference, origin))
            }
            AnnotationType::AnnotationArray(_) => unreachable!("array branch is handled above"),
        };
        self.metadata_case(result, name, vec![payload], origin)
    }
}
