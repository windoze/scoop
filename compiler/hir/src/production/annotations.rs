use super::definition_sources::project_definition_source;
use crate::{
    AnnotatedTargetV1, AnnotationApplicationV1, AnnotationDataBuildError, AnnotationDeclarationV1,
    AnnotationParameterV1, AnnotationTargetV1, CanonicalAnnotationsV1, ExportHir,
    HirDefinitionSourceProjectionError, HirSignatureTypeMapper, HirSignatureTypeMappingError,
};
use scoop_identity::CanonicalIdentifier;
mod targets;
use std::fmt;

impl CanonicalAnnotationsV1 {
    pub fn from_export_hir(
        export: &ExportHir,
        nominals: &crate::CanonicalNominalInterfacesV1,
        properties: &crate::CanonicalPropertyInterfacesV1,
        callables: &crate::CanonicalCallableInterfacesV1,
        aliases: &crate::CanonicalTypeAliasInterfacesV1,
    ) -> Result<Self, AnnotationProductionError> {
        let mapper = HirSignatureTypeMapper::new(crate::HirTypeIdentityInputs::from_export(export));
        let mut declarations = Vec::new();
        for declaration in &export.annotations.declarations {
            if declaration.identity.key().origin() != export.cone {
                continue;
            }
            let parameters = declaration
                .parameters
                .iter()
                .map(|parameter| {
                    let value_type = match parameter.value_type {
                        crate::AnnotationParameterType::Value(ty) => {
                            crate::AnnotationParameterType::Value(
                                mapper
                                    .map(ty, &[])
                                    .map_err(AnnotationProductionError::Type)?,
                            )
                        }
                        crate::AnnotationParameterType::Annotation(id) => {
                            crate::AnnotationParameterType::Annotation(id)
                        }
                        crate::AnnotationParameterType::AnnotationArray { array, element } => {
                            crate::AnnotationParameterType::AnnotationArray { array, element }
                        }
                    };
                    Ok(AnnotationParameterV1 {
                        name: CanonicalIdentifier::new(&parameter.name)
                            .expect("source parameter names are canonical"),
                        value_type,
                        default: parameter.default.clone(),
                    })
                })
                .collect::<Result<_, _>>()?;
            declarations.push(AnnotationDeclarationV1 {
                annotation: declaration.identity.id(),
                parameters,
                visibility: declaration.visibility.into(),
                definition_origin: project_definition_source(export, declaration.definition_origin)
                    .map_err(AnnotationProductionError::Origin)?,
            });
        }
        let retained_targets = AnnotationTargetV1::source_targets(
            nominals,
            properties,
            callables,
            aliases,
            &declarations,
        );
        let mut targets = Vec::new();
        for annotated in &export.annotations.targets {
            let applications = annotated
                .annotations
                .iter()
                .filter(|application| {
                    export.source_files[application.definition_origin.file as usize]
                        .identity
                        .cone()
                        == export.cone
                })
                .map(|application| {
                    Ok(AnnotationApplicationV1 {
                        annotation: application.annotation,
                        arguments: application.arguments.clone(),
                        definition_origin: project_definition_source(
                            export,
                            application.definition_origin,
                        )
                        .map_err(AnnotationProductionError::Origin)?,
                    })
                })
                .collect::<Result<Vec<_>, AnnotationProductionError>>()?;
            if applications.is_empty() {
                continue;
            }
            let target = targets::project(export, annotated.target)?;
            if !retained_targets.contains(&target)
                && !target.is_lexical_callable()
                && !matches!(target, AnnotationTargetV1::TypeAlias(_))
            {
                continue;
            }
            targets.push(AnnotatedTargetV1 {
                target,
                annotations: applications,
            });
        }
        Self::try_new(declarations, targets).map_err(AnnotationProductionError::Table)
    }
}

#[derive(Debug)]
pub enum AnnotationProductionError {
    Type(HirSignatureTypeMappingError),
    Origin(HirDefinitionSourceProjectionError),
    Table(AnnotationDataBuildError),
    InvalidCallable,
}
impl fmt::Display for AnnotationProductionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Type(error) => error.fmt(formatter),
            Self::Origin(error) => error.fmt(formatter),
            Self::Table(error) => error.fmt(formatter),
            Self::InvalidCallable => {
                formatter.write_str("annotation target is not a source callable")
            }
        }
    }
}
impl std::error::Error for AnnotationProductionError {}
