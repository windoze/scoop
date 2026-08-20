use super::*;
use crate::{ExternalHirTargetV1, SignatureNominalWalker, SourceNominalId};
use scoop_wire::{WireError, WirePath};

impl CanonicalAnnotationsV1 {
    pub(crate) fn declaration_targets(
        &self,
        path: &WirePath,
    ) -> Result<Vec<ExternalHirTargetV1>, WireError> {
        let mut references = Vec::new();
        let mut values = Vec::new();
        for declaration in &self.declarations {
            for parameter in &declaration.parameters {
                match &parameter.value_type {
                    AnnotationParameterType::Value(ty) => {
                        type_references(ty, path, &mut references)?
                    }
                    AnnotationParameterType::Annotation(id) => {
                        references.push(ExternalHirTargetV1::Annotation(*id))
                    }
                    AnnotationParameterType::AnnotationArray { array, element } => {
                        references.push(ExternalHirTargetV1::Nominal(
                            SourceNominalId::GenericTemplate(*array),
                        ));
                        references.push(ExternalHirTargetV1::Annotation(*element));
                    }
                }
                values.extend(parameter.default.as_ref());
            }
        }
        for target in &self.targets {
            for application in &target.annotations {
                references.push(ExternalHirTargetV1::Annotation(application.annotation));
                values.extend(&application.arguments);
            }
        }
        for usage in &self.type_uses {
            match &usage.ty {
                AnnotationParameterType::Value(ty) => type_references(ty, path, &mut references)?,
                AnnotationParameterType::Annotation(id) => {
                    references.push(ExternalHirTargetV1::Annotation(*id))
                }
                AnnotationParameterType::AnnotationArray { array, element } => {
                    references.push(ExternalHirTargetV1::Nominal(
                        SourceNominalId::GenericTemplate(*array),
                    ));
                    references.push(ExternalHirTargetV1::Annotation(*element));
                }
            }
            for node in &usage.nodes {
                for application in &node.annotations {
                    references.push(ExternalHirTargetV1::Annotation(application.annotation));
                    values.extend(&application.arguments);
                }
            }
        }
        while let Some(value) = values.pop() {
            match value {
                CanonicalAnnotationValueV1::Scalar(_) => {}
                CanonicalAnnotationValueV1::IntegralEnum {
                    value_type,
                    variant,
                    ..
                } => {
                    type_references(value_type, path, &mut references)?;
                    references.push(ExternalHirTargetV1::Callable(
                        crate::CallableDeclarationId::VariantConstructor(*variant),
                    ));
                }
                CanonicalAnnotationValueV1::Annotation {
                    annotation,
                    arguments,
                } => {
                    references.push(ExternalHirTargetV1::Annotation(*annotation));
                    values.extend(arguments);
                }
                CanonicalAnnotationValueV1::Array {
                    element_type,
                    elements,
                } => {
                    match element_type {
                        AnnotationElementTypeV1::Scalar(_) => {}
                        AnnotationElementTypeV1::IntegralEnum(ty) => {
                            type_references(ty, path, &mut references)?
                        }
                        AnnotationElementTypeV1::Annotation(id) => {
                            references.push(ExternalHirTargetV1::Annotation(*id))
                        }
                    }
                    values.extend(elements.iter().map(|e| &e.value));
                }
            }
        }
        Ok(references)
    }
}

fn type_references(
    ty: &SignatureTypeKey,
    path: &WirePath,
    references: &mut Vec<ExternalHirTargetV1>,
) -> Result<(), WireError> {
    let mut walker = SignatureNominalWalker::new(ty, path)?;
    while let Some(owner) = walker.next(path)? {
        references.push(ExternalHirTargetV1::Nominal(owner));
    }
    Ok(())
}
