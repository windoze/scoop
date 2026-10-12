//! Inner classes retain one ordinary field with the complete outer application.

use super::*;

mod receivers;

impl Lowerer {
    pub(crate) fn enclosing_class_field(&self, class: ClassId) -> Option<hir::ClassFieldId> {
        self.classes[class].fields.iter().copied().find(|field| {
            self.class_fields[*field].source == hir::ClassFieldSource::EnclosingInstance
        })
    }

    pub(crate) fn nominal_enclosing_owner(
        &self,
        owner: hir::SourceNominalId,
    ) -> Option<hir::SourceNominalId> {
        if let Some(source) = self.nominal_owners.get(&owner) {
            let Owner::Class(class) = *source else {
                return None;
            };
            let field = self.enclosing_class_field(class)?;
            let Type::Class(application) = self.types[self.class_field_definition(field).ty] else {
                unreachable!("an enclosing instance has a class type")
            };
            return Some(self.class_applications[application].template);
        }
        let declaration = self.dependencies.as_ref()?.nominal_declaration(owner)?;
        let index = declaration
            .field_sources
            .iter()
            .position(|field| field.storage == hir::NominalFieldStorage::EnclosingInstance)?;
        match declaration.interface.source_shape().declared_fields()[index].value_type() {
            scoop_identity::SignatureTypeKey::Nominal(id) => {
                Some(hir::SourceNominalId::Concrete(*id))
            }
            scoop_identity::SignatureTypeKey::NominalApplication { origin, .. } => {
                Some(hir::SourceNominalId::GenericTemplate(*origin))
            }
            _ => unreachable!("an imported enclosing field has its validated class application"),
        }
    }

    pub(in crate::class) fn enclosing_constructor_parameter(
        &mut self,
        class: ClassId,
        span: ast::Span,
    ) -> Option<hir::ConstructorContext> {
        if let Some(field) = self.captured_environment_field(class) {
            let ty = self.class_field_definition(field).ty;
            return Some(hir::ConstructorContext::CapturedEnvironment(
                self.constructor_parameter("$captures".into(), ty, span),
            ));
        }
        let ty = self
            .class_field_definition(self.enclosing_class_field(class)?)
            .ty;
        Some(hir::ConstructorContext::EnclosingInstance(
            self.constructor_parameter("$outer".into(), ty, span),
        ))
    }

    pub(crate) fn enclosing_parameter_count(&self, class: ClassId) -> usize {
        let Some(field) = self.enclosing_class_field(class) else {
            return 0;
        };
        let Type::Class(application) = self.types[self.class_field_definition(field).ty] else {
            unreachable!("an enclosing instance has a class type")
        };
        self.class_applications[application].arguments.len()
    }
}
