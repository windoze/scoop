use super::*;

mod members;
use members::InterfaceMethodInstance;

impl Concretizer<'_> {
    pub(super) fn ensure_interface(
        &mut self,
        source_id: export::InterfaceId,
        arguments: Vec<concrete::TypeId>,
    ) -> concrete::InterfaceId {
        let key = (source_id, arguments.clone());
        if let Some(&id) = self.interface_by_key.get(&key) {
            return id;
        }
        let source = self.source.interfaces[source_id].clone();
        assert_eq!(source.type_params.len(), arguments.len());
        let name = self.source_nominal_name(&source.name, source.owner);
        let id = concrete::InterfaceId::from_raw(
            u32::try_from(self.interfaces.len())
                .expect("concrete interface ids fit in u32")
                .into(),
        );
        let ty = self.intern_type(concrete::TypeKind::Interface(id), false);
        let allocated = self.interfaces.alloc(concrete::InterfaceDef {
            origin: self.source.nominal_identities[source_id].clone(),
            canonical_type: ty,
            name,
            owner: self.lower_nominal_owner(source.owner),
            family: concrete::InterfaceFamilyId::from_raw(source_id.into_raw().into_u32()),
            type_arguments: arguments.clone(),
            parents: Vec::new(),
            methods: Vec::new(),
            span: source.span,
        });
        assert_eq!(allocated, id);
        self.interface_by_key.insert(key, id);
        self.interface_type.insert(id, ty);
        self.interfaces[id].parents = source
            .parents
            .iter()
            .map(|parent| self.lower_type(*parent, &arguments))
            .collect();
        let source_ty = self.source.interface_applications[source.self_application].canonical_type;
        let method_instances = self.interface_method_instances(source_ty, &arguments);
        let methods = method_instances
            .iter()
            .enumerate()
            .map(|(index, instance)| {
                self.interface_slot_by_source.insert(
                    (id, instance.slot(self.source)),
                    concrete::InterfaceMethodSlot::from_raw(index as u32),
                );
                let (method, method_arguments) = match instance {
                    InterfaceMethodInstance::Local { member, arguments } => (member, arguments),
                    InterfaceMethodInstance::Imported(method) => {
                        return self.lower_imported_interface_method(method, &arguments);
                    }
                };
                let function =
                    &self.source.functions[self.source.interface_methods[*method].function];
                let implementation = match self.source.interface_methods[*method].implementation {
                    export::InterfaceMemberImplementation::Body => {
                        concrete::InterfaceMemberImplementation::Body
                    }
                    export::InterfaceMemberImplementation::AbstractSlot => {
                        concrete::InterfaceMemberImplementation::AbstractSlot
                    }
                };
                concrete::MethodSig {
                    name: function
                        .name
                        .rsplit('.')
                        .next()
                        .expect("interface methods are qualified")
                        .to_string(),
                    is_suspend: function.is_suspend,
                    attributes: function.attributes,
                    implementation,
                    params: function
                        .params
                        .iter()
                        .skip(1)
                        .map(|param| concrete::Param {
                            name: param.name.clone(),
                            ty: self.lower_type(param.ty, method_arguments),
                            local: remap_idx(param.local),
                        })
                        .collect(),
                    return_ty: self.lower_type(function.return_ty, method_arguments),
                    span: function.span,
                }
            })
            .collect();
        self.interfaces[id].methods = methods;
        if source.type_params.is_empty() {
            for member in source.methods {
                let member = &self.source.interface_methods[member];
                self.request_method(
                    member.function,
                    concrete::MethodOwner::Interface(id),
                    MethodRequest::Plain,
                );
            }
        }
        id
    }
}
