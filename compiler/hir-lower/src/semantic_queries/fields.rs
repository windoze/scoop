//! Project original storage declarations through the existing substitution.

use super::*;

mod variants;

impl Lowerer {
    pub(super) fn query_nominal_fields(
        &mut self,
        ty: hir::TypeId,
    ) -> Result<Option<hir::SemanticNominalFields>, Diagnostic> {
        let Some(application) = self.nominal_application(ty) else {
            return Ok(None);
        };
        let parameters = self
            .nominal_declaration_parameters(application.template)
            .expect("a resolved nominal retains complete declaration parameters");
        let bindings = parameters
            .iter()
            .zip(&application.arguments)
            .map(|(parameter, argument)| (parameter.id, *argument))
            .collect::<Vec<_>>();
        let dependency = self
            .dependency_nominal_application(ty)
            .map(|(declaration, _)| declaration.clone());
        let fields = match self.types[ty] {
            hir::Type::Struct(id) => {
                self.query_struct_fields(id, &bindings, dependency.as_deref())?
            }
            hir::Type::Class(id) => {
                self.query_class_fields(id, &bindings, dependency.as_deref())?
            }
            _ => Vec::new(),
        };
        let variants = match self.types[ty] {
            hir::Type::Enum(id) => {
                self.query_enum_variants(id, &bindings, dependency.as_deref())?
            }
            _ => Vec::new(),
        };
        Ok(Some(hir::SemanticNominalFields {
            declaration: self
                .nominal_target_for_type(ty)
                .map_or(application.template, |target| {
                    self.nominal_identity(target.owner()).declaration_id()
                }),
            fields,
            variants,
        }))
    }

    fn query_struct_fields(
        &mut self,
        application: hir::StructApplicationId,
        bindings: &[(hir::TypeParamId, hir::TypeId)],
        dependency: Option<&hir::ImportedNominalDeclaration>,
    ) -> Result<Vec<hir::SemanticFieldDescription>, Diagnostic> {
        let template = self.struct_applications[application].template;
        let fields = self.struct_definition(template).semantic_fields().to_vec();
        fields
            .into_iter()
            .enumerate()
            .map(|(index, field)| {
                let index = u32::try_from(index).expect("source field arity fits u32");
                let hir::FieldRef::StructField {
                    field: identity, ..
                } = self.struct_field_reference(application, index)
                else {
                    unreachable!("a struct field retains its original declaration")
                };
                let (storage, annotations, origin) = if let Some(dependency) = dependency {
                    let source = &dependency.field_sources[index as usize];
                    (
                        source.storage,
                        hir::SemanticAnnotationTarget::Imported(hir::AnnotationTargetV1::Field(
                            identity,
                        )),
                        imported_location(source.origin.origin())?,
                    )
                } else {
                    let structure = self.struct_id(template);
                    let reference = hir::StructFieldRef::checked(&self.structs, structure, index)
                        .expect("the field comes from its actual declaration");
                    (
                        hir::NominalFieldStorage::Declared,
                        hir::SemanticAnnotationTarget::Current(hir::SourceAnnotationTarget::Field(
                            reference,
                        )),
                        self.query_member_location(
                            Owner::Struct(structure),
                            self.struct_field_spans[&reference],
                        ),
                    )
                };
                Ok(hir::SemanticFieldDescription {
                    identity: hir::StaticFieldIdentity::Field(identity),
                    name: field.name,
                    value_type: self.query_type_use(field.ty, bindings),
                    storage,
                    annotations,
                    origin,
                })
            })
            .collect()
    }

    fn query_class_fields(
        &mut self,
        application: hir::ClassApplicationId,
        bindings: &[(hir::TypeParamId, hir::TypeId)],
        dependency: Option<&hir::ImportedNominalDeclaration>,
    ) -> Result<Vec<hir::SemanticFieldDescription>, Diagnostic> {
        let template = self.class_applications[application].template;
        let fields = self.class_definition(template).fields.clone();
        fields
            .into_iter()
            .enumerate()
            .map(|(index, field)| {
                let (identity, storage, annotations, origin) = if let Some(dependency) = dependency
                {
                    let identity =
                        dependency.interface.source_shape().declared_fields()[index].field();
                    let source = &dependency.field_sources[index];
                    (
                        identity,
                        source.storage,
                        hir::SemanticAnnotationTarget::Imported(hir::AnnotationTargetV1::Field(
                            identity,
                        )),
                        imported_location(source.origin.origin())?,
                    )
                } else {
                    let class = self.class_id(template);
                    let reference = self.classes[class].fields[index];
                    let identity = self.class_field_identity(reference);
                    let record = self
                        .field_identity_builder
                        .class_record(reference)
                        .expect("reading an actual class field establishes its identity once");
                    (
                        identity,
                        hir::NominalFieldStorage::from_key(record.key()),
                        hir::SemanticAnnotationTarget::Current(
                            hir::SourceAnnotationTarget::ClassField(reference),
                        ),
                        self.query_member_location(
                            Owner::Class(class),
                            self.class_fields[reference].span,
                        ),
                    )
                };
                Ok(hir::SemanticFieldDescription {
                    identity: hir::StaticFieldIdentity::Field(identity),
                    name: field.name,
                    value_type: self.query_type_use(field.ty, bindings),
                    storage,
                    annotations,
                    origin,
                })
            })
            .collect()
    }

    pub(super) fn query_type_use(
        &mut self,
        declaration: hir::TypeId,
        bindings: &[(hir::TypeParamId, hir::TypeId)],
    ) -> hir::SemanticTypeUse {
        hir::SemanticTypeUse {
            declaration,
            applied: self.instantiate_method_ty(declaration, bindings),
        }
    }

    fn query_member_location(&self, owner: Owner, span: Span) -> hir::SemanticSourceLocation {
        let file = self
            .nominal_definition_file(owner)
            .expect("source members retain their definition file");
        hir::SemanticSourceLocation {
            source: self.source_identity(file).clone(),
            span,
        }
    }
}
