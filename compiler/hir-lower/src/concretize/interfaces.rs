use super::*;

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
            methods: Vec::new(),
            span: source.span,
        });
        assert_eq!(allocated, id);
        self.interface_by_key.insert(key, id);
        self.interface_type.insert(id, ty);
        let method_instances = self.interface_method_instances(source.self_application, &arguments);
        let methods = method_instances
            .iter()
            .enumerate()
            .map(|(index, (method, method_arguments))| {
                self.interface_slot_by_source.insert(
                    (id, *method),
                    concrete::InterfaceMethodSlot::from_raw(index as u32),
                );
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

    pub(super) fn request_abstract_interface_member(
        &mut self,
        application: export::InterfaceApplicationId,
        member: export::InterfaceMethodId,
        substitution: &[concrete::TypeId],
    ) -> concrete::FunctionId {
        let arguments = self
            .interface_method_instances(application, substitution)
            .into_iter()
            .find(|(candidate, _)| *candidate == member)
            .expect("the conformance member belongs to its interface")
            .1;
        let function = self.source.interface_methods[member].function;
        let owner = self.source.functions[function]
            .method
            .expect("an interface declaration has a method owner")
            .owner;
        let ty = self.lower_type(owner, &arguments);
        let concrete::TypeKind::Interface(owner) = self.types[ty].kind else {
            unreachable!("an interface declaration has an interface receiver")
        };
        self.request_method(
            function,
            concrete::MethodOwner::Interface(owner),
            MethodRequest::Plain,
        )
    }

    pub(super) fn interface_method_instances(
        &mut self,
        application: export::InterfaceApplicationId,
        substitution: &[concrete::TypeId],
    ) -> Vec<(export::InterfaceMethodId, Vec<concrete::TypeId>)> {
        let mut result = Vec::new();
        let mut seen = Vec::new();
        self.collect_interface_method_instances(application, substitution, &mut seen, &mut result);
        let suppressed = result
            .iter()
            .flat_map(|(member, _)| {
                self.source.interface_methods[*member]
                    .overrides
                    .iter()
                    .copied()
            })
            .collect::<Vec<_>>();
        result.retain(|(member, _)| !suppressed.contains(member));
        result
    }

    pub(super) fn collect_interface_method_instances(
        &mut self,
        application: export::InterfaceApplicationId,
        substitution: &[concrete::TypeId],
        seen: &mut Vec<(export::InterfaceId, Vec<concrete::TypeId>)>,
        out: &mut Vec<(export::InterfaceMethodId, Vec<concrete::TypeId>)>,
    ) {
        let application = self.source.interface_applications[application].clone();
        let arguments = application
            .arguments
            .iter()
            .map(|argument| self.lower_type(*argument, substitution))
            .collect::<Vec<_>>();
        let key = (application.template, arguments.clone());
        if seen.contains(&key) {
            return;
        }
        seen.push(key);
        let declaration = self.source.interfaces[application.template].clone();
        for parent in declaration.parents {
            self.collect_interface_method_instances(parent, &arguments, seen, out);
        }
        out.extend(
            declaration
                .methods
                .iter()
                .map(|&member| (member, arguments.clone())),
        );
    }
}
