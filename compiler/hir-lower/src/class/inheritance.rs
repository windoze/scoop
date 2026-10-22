use super::*;
mod abstract_methods;
mod coding;
mod context;
mod decoding;
mod delegation;
pub(crate) use delegation::InterfaceForwarder;
mod encoding;
mod imported;
mod inspection;
mod interfaces;
pub(super) use interfaces::InterfaceMemberInstance;
mod order;
mod overrides;
mod properties;
mod results;
mod signatures;

impl Lowerer {
    /// Inheritance checks (pass 2.75): cycles, property shadowing,
    /// `override` rules and interface implementation — for classes and
    /// for value types implementing interfaces (spec 4.4.3).
    pub(crate) fn check_inheritance(
        &mut self,
        pending_classes: &[(ClassId, ast::ClassSource<'_>, usize)],
        pending_structs: &[(hir::StructId, ast::StructSource<'_>, usize)],
        pending_enums: &[(hir::EnumId, &ast::EnumDecl, usize)],
        pending_interfaces: &[(hir::InterfaceId, &ast::InterfaceDecl, usize)],
        pending_objects: &[(hir::ObjectId, crate::declarations::ObjectSource<'_>, usize)],
        pending_methods: &[crate::declarations::MethodHeaderInput<'_>],
    ) {
        let owners = pending_interfaces
            .iter()
            .map(|&(id, _, file)| (Owner::Interface(id), file))
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
        let accessors = self.property_accessor_sources.clone();
        for (owner, file) in owners {
            self.current_file = file;
            self.current_owner = Some(owner);
            let before_delegation = self.diagnostics.len();
            let forwarders = self.declare_interface_forwarders(owner);
            let delegation_failed = self.diagnostics.len() != before_delegation;
            self.check_owner_properties(owner);
            for source in accessors
                .iter()
                .filter(|source| source.owner == Some(owner))
            {
                let mut declaration = overrides::MemberOverride::source(&source.declaration);
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
            for &(id, source, _, _) in pending_methods
                .iter()
                .filter(|(_, _, _, candidate)| *candidate == owner)
            {
                let declaration = overrides::MemberOverride::source(source.signature);
                let before = self.diagnostics.len();
                self.check_member_access_contract(id, &declaration, owner);
                self.check_override_rules(id, &declaration, owner);
                if source.is_override && self.diagnostics.len() != before {
                    self.invalid_override_methods.insert(id);
                }
            }
            for function in forwarders {
                self.check_interface_forwarder(owner, function);
            }
            self.check_inherited_context_contracts(owner);
            if delegation_failed {
                continue;
            }
            match owner {
                Owner::Class(class) => self.check_interface_implementation(
                    class,
                    self.classes[class].span,
                    &owner.describe(self),
                ),
                Owner::Object(object) => self.check_interface_implementation(
                    self.objects[object].backing_class,
                    self.objects[object].span,
                    &owner.describe(self),
                ),
                Owner::Struct(structure) => {
                    self.check_value_interface_implementation(owner, self.structs[structure].span)
                }
                Owner::Enum(enumeration) => {
                    self.check_value_interface_implementation(owner, self.enums[enumeration].span)
                }
                Owner::Interface(_) => {}
            }
        }
        self.current_owner = None;
    }

    pub(crate) fn check_class_inheritance_cycles(
        &mut self,
        pending_classes: &[(ClassId, ast::ClassSource<'_>, usize)],
        pending_objects: &[(hir::ObjectId, crate::declarations::ObjectSource<'_>, usize)],
    ) -> bool {
        let errors = self.diagnostics.len();
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
        self.current_owner = None;
        self.diagnostics.len() == errors
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
