use super::*;

impl Lowerer {
    pub(super) fn check_nominal_interface_implementation(
        &mut self,
        owner: Owner,
        span: ast::Span,
        host: &str,
    ) {
        let parameters = self.owner_type_params(owner);
        let saved_parameters = std::mem::replace(&mut self.type_params_in_scope, parameters);
        let blocks = self.owner_conditional_interfaces(owner).to_vec();
        let interfaces = self.unconditional_owner_interfaces(owner);
        let inherited = self.inherited_conditional_interface_domains(owner);
        let relisted = self.check_inherited_conditional_relisting(owner, &interfaces, &inherited);
        let mut conformances = self.collect_nominal_interface_implementations(
            owner,
            span,
            host,
            &interfaces,
            InterfaceImplementationContext::Unconditional,
        );
        for (interface, condition) in inherited {
            if self.conditional_application_is_impossible(&condition) {
                continue;
            }
            let mut implementations =
                self.with_conditional_interface_assumptions(&condition, |state| {
                    let context = relisted
                        .iter()
                        .find(|(_, target)| *target == interface)
                        .map_or(InterfaceImplementationContext::Inherited, |(id, _)| {
                            InterfaceImplementationContext::Declared(*id)
                        });
                    state.collect_nominal_interface_implementations(
                        owner,
                        span,
                        host,
                        &[interface],
                        context,
                    )
                });
            for implementation in &mut implementations {
                implementation.condition = Some(condition.clone());
            }
            conformances.extend(implementations);
        }
        for block in blocks {
            if relisted.iter().any(|(id, _)| *id == block.id()) {
                continue;
            }
            self.type_params_in_scope = block.parameters.clone();
            let mut interfaces = Vec::new();
            self.append_interface_closure(block.interface, &mut interfaces);
            let mut implementations = self.collect_nominal_interface_implementations(
                owner,
                block.span,
                host,
                &interfaces,
                InterfaceImplementationContext::Declared(block.id()),
            );
            for implementation in &mut implementations {
                implementation.condition = Some(hir::ConditionalInterfaceApplication {
                    declaration: block.id(),
                    owner: block.key.owner,
                    arguments: self.owner_type_args(owner),
                });
            }
            conformances.extend(implementations);
        }
        self.type_params_in_scope = saved_parameters;
        match owner {
            Owner::Class(id) => self.classes[id].interface_implementations = conformances,
            Owner::Object(id) => {
                self.classes[self.objects[id].backing_class].interface_implementations =
                    conformances;
            }
            Owner::Struct(id) => self.structs[id].interface_implementations = conformances,
            Owner::Enum(id) => self.enums[id].interface_implementations = conformances,
            Owner::Interface(_) => unreachable!("interfaces do not carry nominal conformances"),
        }
    }

    pub(super) fn bind_synthesized_interface_condition(
        &mut self,
        function: FunctionId,
        owner: Owner,
        condition: hir::ConditionalInterfaceId,
    ) {
        let block = self
            .owner_conditional_interfaces(owner)
            .iter()
            .find(|block| block.id() == condition)
            .expect("a synthesized implementation belongs to its original block");
        let parameters = block.parameters.clone();
        self.functions[function].span = block.span;
        self.functions[function]
            .method
            .as_mut()
            .expect("a synthesized implementation is a member")
            .availability = Some(condition);
        self.signatures.get_mut(&function).unwrap().type_params = parameters.clone();
        match &mut self.functions[function].genericity {
            hir::FunctionGenericity::OwnerParameterizedMethod {
                owner_parameters, ..
            } => {
                *owner_parameters = parameters;
            }
            hir::FunctionGenericity::Plain => {
                assert!(parameters.is_empty());
            }
            _ => unreachable!("synthesized interface members have no method parameters"),
        }
    }

    fn unconditional_owner_interfaces(&mut self, owner: Owner) -> Vec<TypeId> {
        let (mut roots, class) = match owner {
            Owner::Class(id) => (self.classes[id].interfaces.clone(), Some(id)),
            Owner::Object(id) => {
                let class = self.objects[id].backing_class;
                (self.classes[class].interfaces.clone(), Some(class))
            }
            Owner::Struct(id) => (self.structs[id].interfaces.clone(), None),
            Owner::Enum(id) => {
                let mut roots = self.enums[id].interfaces.clone();
                if let Some(kind) = self.enums[id].representation.integer_kind() {
                    roots.extend(
                        self.intrinsic_type_interfaces(hir::IntrinsicTypeKind::Integer(kind)),
                    );
                }
                (roots, None)
            }
            Owner::Interface(_) => unreachable!("conformance owners are ordinary types"),
        };
        if let Some(base) = class.and_then(|class| self.classes[class].base_class) {
            let Type::Class(base) = self.types[base] else {
                unreachable!("a checked class base has a class application")
            };
            let base = self.class_applications[base].clone();
            let inherited = self
                .class_definition(base.template)
                .interface_implementations
                .clone();
            roots.extend(
                inherited
                    .into_iter()
                    .filter(|implementation| implementation.condition.is_none())
                    .map(|implementation| {
                        self.instantiate_ty(implementation.interface, &base.arguments)
                    }),
            );
        }
        let mut interfaces = Vec::new();
        for root in roots {
            self.append_interface_closure(root, &mut interfaces);
        }
        interfaces
    }
}
