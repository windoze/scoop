//! Proofs read checked declarations, including while a concrete table is being built.

use super::*;

struct NominalRelations<'a> {
    ordinary: Vec<export::TypeId>,
    conditional: &'a [export::ConditionalInterfaceDeclaration],
    arguments: Vec<concrete::TypeId>,
}

impl Concretizer<'_> {
    pub(super) fn concrete_subtype(
        &mut self,
        source: concrete::TypeId,
        target: concrete::TypeId,
    ) -> bool {
        if source == target
            || self.is_nothing_type(source)
            || matches!(self.types[target].kind, concrete::TypeKind::Any)
        {
            return true;
        }
        if !self.active_conditional_relations.insert((source, target)) {
            return false;
        }
        let result = self.concrete_subtype_from_declaration(source, target);
        self.active_conditional_relations.remove(&(source, target));
        result
    }

    fn concrete_subtype_from_declaration(
        &mut self,
        source: concrete::TypeId,
        target: concrete::TypeId,
    ) -> bool {
        if let Some(relations) = self.concrete_nominal_relations(source) {
            for parent in relations.ordinary {
                let parent = self.lower_type(parent, &relations.arguments);
                if self.concrete_subtype(parent, target) {
                    return true;
                }
            }
            if matches!(self.types[target].kind, concrete::TypeKind::Interface(_)) {
                for declaration in relations.conditional {
                    // First establish the guard; its target can contain a
                    // companion projection whose existence is conditional.
                    if self.concrete_conditional_parameters_hold(
                        &declaration.parameters,
                        &relations.arguments,
                    ) {
                        let interface =
                            self.lower_type(declaration.interface, &relations.arguments);
                        if self.concrete_subtype(interface, target) {
                            return true;
                        }
                    }
                }
            }
            return false;
        }
        let concrete::TypeKind::Interface(interface) = self.types[target].kind else {
            return false;
        };
        match self.types[source].kind {
            concrete::TypeKind::Interface(_) => {
                self.concrete_interface_closure(&[source]).contains(&target)
            }
            concrete::TypeKind::Any
            | concrete::TypeKind::Tuple(_)
            | concrete::TypeKind::Function(_)
            | concrete::TypeKind::FunPtr(_) => false,
            _ => self
                .concrete_bound_conformances(source)
                .iter()
                .any(|implementation| implementation.interface == interface),
        }
    }
}

impl<'a> Concretizer<'a> {
    fn concrete_nominal_relations(&self, ty: concrete::TypeId) -> Option<NominalRelations<'a>> {
        match self.types[ty].kind {
            concrete::TypeKind::Class(id) => {
                let concrete = &self.classes[id];
                let source = self
                    .source
                    .class_definition(concrete.origin.declaration_id());
                Some(NominalRelations {
                    ordinary: source
                        .base_class
                        .into_iter()
                        .chain(source.interfaces.iter().copied())
                        .collect(),
                    conditional: &source.conditional_interfaces,
                    arguments: concrete.type_arguments.clone(),
                })
            }
            concrete::TypeKind::Struct(id)
                if matches!(
                    self.structs[id].representation,
                    concrete::StructRepresentation::Declared(_)
                ) =>
            {
                let concrete = &self.structs[id];
                let source = self
                    .source
                    .struct_definition(concrete.origin.declaration_id());
                Some(NominalRelations {
                    ordinary: source.interfaces.clone(),
                    conditional: &source.conditional_interfaces,
                    arguments: concrete.type_arguments.clone(),
                })
            }
            concrete::TypeKind::Enum(id) => {
                let concrete = &self.enums[id];
                let source = self
                    .source
                    .enum_definition(concrete.origin.declaration_id());
                let mut ordinary = source.interfaces.clone();
                if let Some(kind) = source.representation.integer_kind() {
                    ordinary.extend_from_slice(self.source.integer_interface_roots(kind));
                }
                Some(NominalRelations {
                    ordinary,
                    conditional: &source.conditional_interfaces,
                    arguments: concrete.type_arguments.clone(),
                })
            }
            _ => None,
        }
    }
}
