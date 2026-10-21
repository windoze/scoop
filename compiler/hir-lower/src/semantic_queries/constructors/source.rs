use super::*;
use crate::call_resolution::candidates::NominalConstructorSource;

impl Lowerer {
    pub(super) fn query_struct_constructors(
        &mut self,
        application: hir::StructApplicationId,
        result: hir::SemanticTypeUse,
        substitutions: &[(hir::TypeParamId, hir::TypeId)],
    ) -> Result<Vec<hir::SemanticConstructorDescription>, Diagnostic> {
        let structure = self.struct_id(self.struct_applications[application].template);
        let constructors = self.structs[structure].constructors.clone();
        constructors
            .into_iter()
            .map(|id| {
                let identity = CallableTemplateOrigin::Constructor(
                    self.query_struct_constructor_identity(id)?.id(),
                );
                let primary = matches!(
                    self.struct_constructors[id].kind,
                    hir::StructConstructorKind::Primary
                );
                let mut parameters = self.query_current_parameters(
                    hir::ExportParameterOwner::StructConstructor(id),
                    identity,
                    substitutions,
                );
                if primary {
                    for (index, parameter) in parameters.iter_mut().enumerate() {
                        let hir::FieldRef::StructField { field, .. } =
                            self.struct_field_reference(application, index as u32)
                        else {
                            unreachable!("a struct primary parameter retains its field")
                        };
                        parameter.target = hir::StaticParameterTarget::Field(
                            hir::StaticFieldIdentity::Field(field),
                        );
                    }
                }
                let value = &self.struct_constructors[id];
                Ok(hir::SemanticConstructorDescription {
                    identity,
                    kind: if primary {
                        hir::SemanticConstructorKind::Primary
                    } else {
                        hir::SemanticConstructorKind::Secondary
                    },
                    parameters,
                    result,
                    access: hir::SemanticMemberAccess {
                        visibility: value.access.declared.into(),
                        domain: value.access.lookup.0.clone(),
                        accessible: self
                            .constructor_is_accessible(NominalConstructorSource::Struct(id)),
                    },
                    annotations: hir::SemanticAnnotationTarget::Current(
                        hir::SourceAnnotationTarget::Callable(
                            hir::SourceAnnotationCallable::StructConstructor(id),
                        ),
                    ),
                    origin: hir::SemanticSourceLocation {
                        source: self.source_identity(value.origin.file as usize).clone(),
                        span: value.span,
                    },
                })
            })
            .collect()
    }

    pub(super) fn query_class_constructors(
        &mut self,
        class: hir::ClassId,
        result: hir::SemanticTypeUse,
        substitutions: &[(hir::TypeParamId, hir::TypeId)],
    ) -> Result<Vec<hir::SemanticConstructorDescription>, Diagnostic> {
        let constructors = self.classes[class].constructors.clone();
        constructors
            .into_iter()
            .filter(|id| {
                self.class_constructors[*id].identity_kind
                    == hir::ClassConstructorIdentityKind::Source
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|id| {
                let identity = CallableTemplateOrigin::Constructor(
                    self.query_class_constructor_identity(id)?.id(),
                );
                let primary = matches!(
                    self.class_constructors[id].kind,
                    hir::ClassConstructorKind::Primary { .. }
                );
                let mut parameters = self.query_current_parameters(
                    hir::ExportParameterOwner::ClassConstructor(id),
                    identity,
                    substitutions,
                );
                if primary {
                    for (index, parameter) in parameters.iter_mut().enumerate() {
                        let target = self.class_constructors[id].parameters[index].id;
                        let property =
                            self.classes[class].fields.iter().find_map(|field| {
                                match self.class_fields[*field].source {
                                    hir::ClassFieldSource::PrimaryParameter {
                                        property,
                                        parameter,
                                    } if parameter == target => Some(property),
                                    _ => None,
                                }
                            });
                        if let Some(property) = property {
                            parameter.target = hir::StaticParameterTarget::Property(
                                self.ordinary_property_identity(property)
                                    .ordinary_id()
                                    .expect(
                                        "a primary property has an ordinary declaration identity",
                                    ),
                            );
                        }
                    }
                }
                let value = &self.class_constructors[id];
                Ok(hir::SemanticConstructorDescription {
                    identity,
                    kind: if primary {
                        hir::SemanticConstructorKind::Primary
                    } else {
                        hir::SemanticConstructorKind::Secondary
                    },
                    parameters,
                    result,
                    access: hir::SemanticMemberAccess {
                        visibility: value.access.declared.into(),
                        domain: value.access.lookup.0.clone(),
                        accessible: self
                            .constructor_is_accessible(NominalConstructorSource::Class(id)),
                    },
                    annotations: hir::SemanticAnnotationTarget::Current(
                        hir::SourceAnnotationTarget::Callable(
                            hir::SourceAnnotationCallable::ClassConstructor(id),
                        ),
                    ),
                    origin: hir::SemanticSourceLocation {
                        source: self.source_identity(value.origin.file as usize).clone(),
                        span: value.span,
                    },
                })
            })
            .collect()
    }
}
