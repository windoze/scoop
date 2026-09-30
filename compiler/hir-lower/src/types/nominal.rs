//! Complete application keys over declaration storage during HIR lowering.

use super::*;
use crate::Owner;

#[derive(Debug, Clone)]
pub(crate) struct NominalApplication {
    pub(crate) ty: TypeId,
    pub(crate) template: hir::SourceNominalId,
    pub(crate) arguments: Vec<TypeId>,
}

impl Lowerer {
    pub(crate) fn dependency_interface_definition(
        &self,
        ty: TypeId,
    ) -> Option<&hir::LoadedInterfaceDefinition> {
        let Type::Interface(application) = self.types[ty] else {
            return None;
        };
        self.loaded_interface_definitions
            .get(&self.interface_applications[application].template)
    }

    pub(crate) fn struct_definition(
        &self,
        template: hir::SourceNominalId,
    ) -> &hir::StructDefinition {
        match self.source_struct_id(template) {
            Some(id) => &self.structs[id].definition,
            None => &self.loaded_struct_definitions[&template].definition,
        }
    }

    pub(crate) fn class_definition(&self, template: hir::SourceNominalId) -> &hir::ClassDefinition {
        match self.source_class_id(template) {
            Some(id) => &self.classes[id].definition,
            None => &self.loaded_class_definitions[&template].definition,
        }
    }

    pub(crate) fn interface_definition(
        &self,
        template: hir::SourceNominalId,
    ) -> &hir::InterfaceDefinition {
        match self.source_interface_id(template) {
            Some(id) => &self.interfaces[id].definition,
            None => &self.loaded_interface_definitions[&template].definition,
        }
    }

    pub(crate) fn class_field_definition(&self, field: hir::ClassFieldId) -> &hir::Field {
        let source = &self.class_fields[field];
        &self.classes[source.owner].definition.fields[source.definition_index]
    }

    pub(crate) fn dependency_nominal_application(
        &self,
        ty: TypeId,
    ) -> Option<(&std::sync::Arc<hir::ImportedNominalDeclaration>, &[TypeId])> {
        if let Type::Struct(application) = self.types[ty] {
            let application = &self.struct_applications[application];
            let definition = self.loaded_struct_definitions.get(&application.template)?;
            return Some((&definition.declaration, &application.arguments));
        }
        if let Type::Class(application) = self.types[ty] {
            let application = &self.class_applications[application];
            let definition = self.loaded_class_definitions.get(&application.template)?;
            return Some((&definition.declaration, &application.arguments));
        }
        if let Type::Enum(application) = self.types[ty] {
            let application = &self.enum_applications[application];
            let definition = self.loaded_enum_definitions.get(&application.template)?;
            return Some((&definition.declaration, &application.arguments));
        }
        if let Type::Interface(application) = self.types[ty] {
            let application = &self.interface_applications[application];
            let definition = self
                .loaded_interface_definitions
                .get(&application.template)?;
            return Some((&definition.declaration, &application.arguments));
        }
        None
    }

    pub(crate) fn nominal_intrinsic_kind(
        &self,
        template: hir::SourceNominalId,
    ) -> Option<hir::IntrinsicTypeKind> {
        if let Some(owner) = self.nominal_owners.get(&template) {
            return match *owner {
                Owner::Struct(id) => match self.structs[id].representation {
                    hir::StructRepresentation::Intrinsic(kind) => Some(kind),
                    hir::StructRepresentation::Declared(_) => None,
                },
                Owner::Class(id) => match self.classes[id].representation {
                    hir::ClassRepresentation::Intrinsic(kind) => Some(kind),
                    hir::ClassRepresentation::Declared => None,
                },
                Owner::Enum(_) | Owner::Interface(_) | Owner::Object(_) => None,
            };
        }
        let declaration = self.dependencies.as_ref()?.nominal_declaration(template)?;
        match declaration.interface.source_shape() {
            hir::NominalSourceShapeV1::Intrinsic(representation) => Some(representation.family()),
            _ => None,
        }
    }

    pub(crate) fn apply_nominal_type(
        &mut self,
        template: hir::SourceNominalId,
        arguments: Vec<TypeId>,
    ) -> Result<TypeId, crate::imported_core::ImportedSignatureTypeError> {
        let Some(owner) = self.nominal_owners.get(&template).copied() else {
            return self.imported_nominal_application(template, arguments);
        };
        Ok(match owner {
            Owner::Struct(id) => self.struct_application(id, arguments),
            Owner::Class(id) => self.class_application(id, arguments),
            Owner::Enum(id) => self.enum_application(id, arguments),
            Owner::Interface(id) => self.source_interface_type(id, arguments),
            Owner::Object(id) => self.class_application(self.objects[id].backing_class, arguments),
        })
    }

    pub(crate) fn nominal_application(&self, ty: TypeId) -> Option<NominalApplication> {
        let (template, arguments) = match self.types[ty] {
            Type::Struct(application) => {
                let application = &self.struct_applications[application];
                (application.template, application.arguments.clone())
            }
            Type::Class(application) => {
                let application = &self.class_applications[application];
                (application.template, application.arguments.clone())
            }
            Type::Enum(application) => {
                let application = &self.enum_applications[application];
                (application.template, application.arguments.clone())
            }
            Type::Interface(application) => {
                let application = &self.interface_applications[application];
                (application.template, application.arguments.clone())
            }
            _ => return None,
        };
        Some(NominalApplication {
            ty,
            template,
            arguments,
        })
    }
    pub(crate) fn nominal_template_name(&self, template: hir::SourceNominalId) -> &str {
        if let Some(owner) = self.nominal_owners.get(&template) {
            return match *owner {
                Owner::Struct(id) => &self.structs[id].name,
                Owner::Class(id) => &self.classes[id].name,
                Owner::Enum(id) => &self.enums[id].name,
                Owner::Interface(id) => &self.interfaces[id].name,
                Owner::Object(id) => &self.objects[id].name,
            };
        }
        self.dependencies
            .as_ref()
            .and_then(|dependencies| dependencies.nominal_declaration(template))
            .expect("a resolved nominal retains its original declaration")
            .name()
    }
}
