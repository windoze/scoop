use super::*;

impl Lowerer {
    pub(super) fn set_integral_box_parent(
        &mut self,
        module: &hir::Module,
        class: mir::ClassId,
        payload: &mir::Type,
    ) {
        let mir::Type::Enum(id, _) = payload else {
            return;
        };
        let Some(kind) = self.enums.defs[*id].representation.integer_kind() else {
            return;
        };
        let source = module.types.iter().find_map(|(id, ty)| {
            matches!(ty.kind, hir::TypeKind::Integer(source) if crate::types::lower_integer_kind(source) == kind).then_some(id)
        }).expect("an integral enum retains its integer parent type");
        let parent_payload = Types {
            module,
            struct_map: &self.struct_map,
            class_map: &self.class_map,
        }
        .lower(
            source,
            &mut self.source_exact_types,
            &mut self.enums,
            &mut self.structs,
            &mut self.interfaces,
            &mut self.shell,
        );
        let parent = self.boxed.get_or_create(
            &mut self.classes,
            &mut self.shell,
            &parent_payload,
            module.exact_type_identities[source].id(),
        );
        for definition in [&mut self.classes[class], &mut self.shell.classes[class]] {
            let mir::ClassRepresentation::Declared(declaration) = &mut definition.representation
            else {
                unreachable!("value boxes have declared storage");
            };
            declaration.base_class = Some(parent);
        }
    }
}
