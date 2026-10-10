use super::definition_sources::project_definition_source;
use crate::{
    AnnotatedTargetV1, AnnotationApplicationV1, AnnotationDataBuildError, AnnotationDeclarationV1,
    AnnotationParameterV1, AnnotationTargetV1, CanonicalAnnotationsV1, ExportHir,
    HirDefinitionSourceProjectionError, HirSignatureTypeMapper, HirSignatureTypeMappingError,
    NominalOwner, SourceAnnotationTarget,
};
use scoop_identity::CanonicalIdentifier;
use std::fmt;

impl CanonicalAnnotationsV1 {
    pub fn from_export_hir(
        export: &ExportHir,
        nominals: &crate::CanonicalNominalInterfacesV1,
        properties: &crate::CanonicalPropertyInterfacesV1,
    ) -> Result<Self, AnnotationProductionError> {
        let retained_targets = AnnotationTargetV1::source_targets(nominals, properties);
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
                    let value_type = mapper
                        .map(parameter.value_type, &[])
                        .map_err(AnnotationProductionError::Type)?;
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
            let target = match annotated.target {
                SourceAnnotationTarget::Nominal(owner) => {
                    let identity = match owner {
                        NominalOwner::Struct(id) => &export.nominal_identities[id],
                        NominalOwner::Enum(id) => &export.nominal_identities[id],
                        NominalOwner::Class(id) => &export.nominal_identities[id],
                        NominalOwner::Interface(id) => &export.nominal_identities[id],
                        NominalOwner::Object(id) => &export.nominal_identities[id],
                    };
                    AnnotationTargetV1::Nominal(identity.declaration_id())
                }
                SourceAnnotationTarget::Field(field) => {
                    AnnotationTargetV1::Field(export.field_identities[field].id())
                }
                SourceAnnotationTarget::Variant(variant) => {
                    AnnotationTargetV1::Variant(export.enum_member_identities[variant].id())
                }
                SourceAnnotationTarget::VariantField(field) => {
                    AnnotationTargetV1::VariantField(export.enum_member_identities[field].id())
                }
                SourceAnnotationTarget::Property(property) => {
                    let crate::HirPropertyIdentity::Ordinary(identity) =
                        &export.property_identities[property]
                    else {
                        return Err(AnnotationProductionError::ExtensionProperty);
                    };
                    AnnotationTargetV1::Property(identity.id())
                }
            };
            if !retained_targets.contains(&target) {
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
    ExtensionProperty,
}
impl fmt::Display for AnnotationProductionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Type(error) => error.fmt(formatter),
            Self::Origin(error) => error.fmt(formatter),
            Self::Table(error) => error.fmt(formatter),
            Self::ExtensionProperty => {
                formatter.write_str("extension properties are not annotation targets")
            }
        }
    }
}
impl std::error::Error for AnnotationProductionError {}
