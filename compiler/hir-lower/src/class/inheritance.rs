use super::*;
mod abstract_methods;
mod coding;
mod context;
mod decoding;
mod encoding;
mod imported;
mod interfaces;
mod order;
mod overrides;
mod properties;
mod signatures;

impl Lowerer {
    /// Inheritance checks (pass 2.75): cycles, property shadowing,
    /// `override` rules and interface implementation — for classes and
    /// for value types implementing interfaces (spec 4.4.3).
    pub(crate) fn check_inheritance(
        &mut self,
        pending_classes: &[(ClassId, &ast::ClassDecl, usize)],
        pending_structs: &[(hir::StructId, &ast::StructDecl, usize)],
        pending_enums: &[(hir::EnumId, &ast::EnumDecl, usize)],
        pending_objects: &[(hir::ObjectId, crate::declarations::ObjectSource<'_>, usize)],
        pending_methods: &[(FunctionId, &ast::FunctionDecl, usize, Owner)],
    ) {
        for &(id, decl, file_index) in pending_classes {
            self.current_file = file_index;
            self.current_owner = Some(Owner::Class(id));
            self.check_inheritance_cycle(id, decl.span, "class");
        }
        for &(object, source, file_index) in pending_objects {
            self.current_file = file_index;
            self.current_owner = Some(Owner::Object(object));
            self.check_inheritance_cycle(
                self.objects[object].backing_class,
                source.span(),
                source.description(),
            );
        }
        let owners = self
            .interfaces
            .iter()
            .map(|(id, _)| (Owner::Interface(id), self.interface_files[&id]))
            .chain(
                pending_classes
                    .iter()
                    .map(|&(id, _, file)| (Owner::Class(id), file)),
            )
            .chain(
                pending_objects
                    .iter()
                    .map(|&(id, _, file)| (Owner::Object(id), file)),
            )
            .chain(
                pending_structs
                    .iter()
                    .map(|&(id, _, file)| (Owner::Struct(id), file)),
            )
            .chain(
                pending_enums
                    .iter()
                    .map(|&(id, _, file)| (Owner::Enum(id), file)),
            )
            .collect::<Vec<_>>();
        let owners = self.source_inheritance_order(owners);
        let order = owners
            .iter()
            .enumerate()
            .map(|(index, &(owner, _))| (owner, index))
            .collect::<std::collections::HashMap<_, _>>();
        for &(owner, file) in &owners {
            self.current_file = file;
            self.current_owner = Some(owner);
            self.check_owner_properties(owner);
        }
        self.current_owner = None;
        let mut accessors = self.property_accessor_sources.clone();
        accessors.sort_by_key(|source| source.owner.map(|owner| order[&owner]));
        for source in accessors {
            let Some(owner) = source.owner else {
                continue;
            };
            self.current_file = self.function_files[&source.function];
            self.current_owner = Some(owner);
            let mut declaration = source.declaration.clone();
            let validated_target = self.properties[source.property]
                .overrides
                .iter()
                .copied()
                .any(|inherited| self.property_has_callable_accessor(inherited, source.kind));
            let declared_target = if source.declaration.is_override {
                let name = self.properties[source.property].name.clone();
                self.inherited_property_candidates(owner, &name)
                    .into_iter()
                    .any(|inherited| {
                        self.property_has_callable_accessor(inherited.reference, source.kind)
                    })
            } else {
                false
            };
            declaration.is_override = validated_target || declared_target;
            self.check_member_access_contract(source.function, &declaration, owner);
            self.check_override_rules(source.function, &declaration, owner);
        }
        self.current_owner = None;
        let mut methods = pending_methods.to_vec();
        methods.sort_by_key(|&(_, _, _, owner)| order[&owner]);
        for (id, decl, file_index, owner) in methods {
            self.current_file = file_index;
            self.current_owner = Some(owner);
            self.check_member_access_contract(id, decl, owner);
            self.check_override_rules(id, decl, owner);
        }
        for &(owner, file) in &owners {
            self.current_file = file;
            self.current_owner = Some(owner);
            self.check_inherited_context_contracts(owner);
        }
        self.current_owner = None;
        for &(id, decl, file_index) in pending_classes {
            self.current_file = file_index;
            self.current_owner = Some(Owner::Class(id));
            self.check_interface_implementation(
                id,
                decl.span,
                &format!("class `{}`", decl.name.text),
            );
        }
        for &(object, source, file_index) in pending_objects {
            self.current_file = file_index;
            self.current_owner = Some(Owner::Object(object));
            self.check_interface_implementation(
                self.objects[object].backing_class,
                source.span(),
                &format!("{} `{}`", source.description(), self.objects[object].name),
            );
        }
        for &(id, decl, file_index) in pending_structs {
            self.current_file = file_index;
            self.current_owner = Some(Owner::Struct(id));
            self.check_value_interface_implementation(Owner::Struct(id), decl.span);
        }
        for &(id, decl, file_index) in pending_enums {
            self.current_file = file_index;
            self.current_owner = Some(Owner::Enum(id));
            self.check_value_interface_implementation(Owner::Enum(id), decl.span);
        }
        self.current_owner = None;
    }

    /// A class may not directly or indirectly inherit from itself.
    fn check_inheritance_cycle(&mut self, id: ClassId, span: ast::Span, host: &str) {
        let mut seen = vec![id];
        let mut current = id;
        while let Some(base_ty) = self.classes[current].base_class {
            let base_application = match self.types[base_ty] {
                Type::Class(application) => application,
                _ => unreachable!("resolved class bases have class types"),
            };
            let base = self.class_applications[base_application].template;
            let Some(base) = self.source_class_id(base) else {
                break;
            };
            if seen.contains(&base) {
                let name = self.classes[id].name.clone();
                self.error(
                    span,
                    format!("{host} `{name}` directly or indirectly inherits from itself"),
                );
                return;
            }
            seen.push(base);
            current = base;
        }
    }
}
