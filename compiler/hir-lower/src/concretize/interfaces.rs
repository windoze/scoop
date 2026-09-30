use super::*;

mod definition;
mod members;

use definition::{InterfaceMembers, ResolvedInterfaceDefinition, ResolvedInterfaceMethod};
use members::InterfaceMethodInstance;

impl Concretizer<'_> {
    pub(super) fn ensure_interface(
        &mut self,
        source_id: export::InterfaceId,
        arguments: Vec<concrete::TypeId>,
    ) -> concrete::InterfaceId {
        let origin = self.source.nominal_identities[source_id].declaration_id();
        if let Some(&id) = self.interface_by_key.get(&(origin, arguments.clone())) {
            return id;
        }
        let source = &self.source.interfaces[source_id];
        let definition = ResolvedInterfaceDefinition {
            origin: self.source.nominal_identities[source_id].clone(),
            name: self.source_nominal_name(&source.name, source.owner),
            owner: self.lower_nominal_owner(source.owner),
            parents: &source.parents,
            members: InterfaceMembers::Declared(
                self.source.interface_applications[source.self_application].canonical_type,
            ),
            automatic_methods: if source.type_params.is_empty() {
                &source.methods
            } else {
                &[]
            },
            span: source.span,
        };
        let id = self.allocate_interface_definition(&definition, arguments.clone());
        self.complete_interface_definition(id, definition, &arguments);
        id
    }

    pub(super) fn lower_imported_interface(
        &mut self,
        source: &export::ImportedInterfaceType,
        substitution: &[concrete::TypeId],
    ) -> concrete::TypeId {
        let arguments = source
            .arguments
            .iter()
            .map(|argument| self.lower_type(*argument, substitution))
            .collect::<Vec<_>>();
        let key = (source.declaration.owner(), arguments.clone());
        if let Some(id) = self.interface_by_key.get(&key) {
            return self.interface_type[id];
        }
        let definition = ResolvedInterfaceDefinition {
            origin: export::HirNominalIdentity::Source(source.declaration.identity.clone()),
            name: source.declaration.name().to_owned(),
            owner: None,
            parents: &source.parents,
            members: InterfaceMembers::Resolved(&source.methods),
            automatic_methods: &[],
            span: scoop_ast::Span::new(0, 0),
        };
        let id = self.allocate_interface_definition(&definition, arguments);
        self.complete_interface_definition(id, definition, substitution);
        self.interface_type[&id]
    }

    fn allocate_interface_definition(
        &mut self,
        definition: &ResolvedInterfaceDefinition<'_>,
        arguments: Vec<concrete::TypeId>,
    ) -> concrete::InterfaceId {
        let origin = definition.origin.declaration_id();
        let id = concrete::InterfaceId::from_raw(
            u32::try_from(self.interfaces.len())
                .expect("concrete interface ids fit in u32")
                .into(),
        );
        let next_family = self.interface_families.len();
        let family = *self.interface_families.entry(origin).or_insert_with(|| {
            concrete::InterfaceFamilyId::from_raw(
                u32::try_from(next_family).expect("concrete interface families fit in u32"),
            )
        });
        let ty = self.intern_type(concrete::TypeKind::Interface(id), false);
        let allocated = self.interfaces.alloc(concrete::InterfaceDef {
            origin: definition.origin.clone(),
            canonical_type: ty,
            name: definition.name.clone(),
            owner: definition.owner.clone(),
            family,
            type_arguments: arguments.clone(),
            parents: Vec::new(),
            methods: Vec::new(),
            span: definition.span,
        });
        assert_eq!(allocated, id);
        self.interface_by_key.insert((origin, arguments), id);
        self.interface_type.insert(id, ty);
        id
    }

    fn complete_interface_definition(
        &mut self,
        id: concrete::InterfaceId,
        definition: ResolvedInterfaceDefinition<'_>,
        substitution: &[concrete::TypeId],
    ) {
        self.interfaces[id].parents = definition
            .parents
            .iter()
            .map(|parent| self.lower_type(*parent, substitution))
            .collect();
        let methods = match definition.members {
            InterfaceMembers::Declared(ty) => self.interface_method_instances(ty, substitution),
            InterfaceMembers::Resolved(methods) => methods
                .iter()
                .map(|method| InterfaceMethodInstance {
                    method: ResolvedInterfaceMethod::from_dependency(method),
                    arguments: substitution.to_vec(),
                })
                .collect(),
        };
        self.interfaces[id].methods = methods
            .iter()
            .enumerate()
            .map(|(index, instance)| {
                self.interface_slot_by_source.insert(
                    (id, instance.method.slot),
                    concrete::InterfaceMethodSlot::from_raw(
                        u32::try_from(index).expect("concrete interface slots fit in u32"),
                    ),
                );
                self.lower_interface_method(&instance.method, &instance.arguments)
            })
            .collect();
        for &member in definition.automatic_methods {
            self.request_method(
                self.source.interface_methods[member].function,
                concrete::MethodOwner::Interface(id),
                Vec::new(),
            );
        }
    }

    fn lower_interface_method(
        &mut self,
        method: &ResolvedInterfaceMethod<'_>,
        substitution: &[concrete::TypeId],
    ) -> concrete::MethodSig {
        concrete::MethodSig {
            name: method.name.to_owned(),
            is_suspend: method.is_suspend,
            attributes: method.attributes,
            implementation: method.implementation,
            params: method
                .parameters
                .iter()
                .map(|&(name, ty, local)| concrete::Param {
                    name: name.to_owned(),
                    ty: self.lower_type(ty, substitution),
                    local,
                })
                .collect(),
            return_ty: self.lower_type(method.return_type, substitution),
            span: method.span,
        }
    }
}
