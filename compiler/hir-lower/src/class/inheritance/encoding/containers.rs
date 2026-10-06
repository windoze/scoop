use super::*;

impl Lowerer {
    pub(crate) fn declare_container_encodings(&mut self) {
        let Some(encodable) = self.core_coding_nominal("Encodable") else {
            return;
        };
        let Some(encoder) = self.core_coding_nominal("Encoder") else {
            return;
        };
        let interface = self.intern_interface_application(encodable, Vec::new());
        let encodable = self.interface_applications[interface].canonical_type;
        let interface = self.intern_interface_application(encoder, Vec::new());
        let encoder = self.interface_applications[interface].canonical_type;
        let members = self.conformance_members(encodable);
        let [member] = members.as_slice() else { return };
        if member.signature.name != "encode"
            || member.signature.parameters != [encoder]
            || member.signature.result != self.unit
        {
            return;
        }
        for name in ["Option", "Array", "MutableArray", "ArrayList"] {
            let Some(source) = self.core_coding_nominal(name) else {
                continue;
            };
            let Some(&owner) = self.nominal_owners.get(&source) else {
                continue;
            };
            let (file, span) = match owner {
                Owner::Enum(id) => (self.enum_files[&id], self.enums[id].span),
                Owner::Class(id) => (self.class_files[&id], self.classes[id].span),
                _ => continue,
            };
            self.current_file = file;
            let parameters = self.owner_type_params(owner);
            let [parameter] = parameters.as_slice() else {
                self.error(
                    span,
                    "core container encoding requires one element type parameter".into(),
                );
                continue;
            };
            let element = self.intern_type(Type::Param(parameter.id));
            let mut parameter = parameter.clone();
            parameter.id = self.fresh_type_param(0);
            parameter.bounds = hir::TypeParamBounds::Nominal(hir::NominalBounds {
                class: None,
                interfaces: vec![hir::InterfaceUpperBound {
                    ty: encodable,
                    span,
                }],
            });
            let bounded_element = self.intern_type(Type::Param(parameter.id));
            let receiver = self.owner_ty(owner);
            let receiver = self.instantiate_ty(receiver, &[bounded_element]);
            let function =
                self.register_coding_method(owner, "encode", "encoder", encoder, self.unit);
            self.register_method_parameters(function, vec![parameter.clone()], Vec::new());
            self.signatures
                .get_mut(&function)
                .expect("registered method")
                .type_params = vec![parameter];
            self.functions[function]
                .method
                .as_mut()
                .expect("ordinary encode method")
                .owner = receiver;
            let owner_application = self.method_owner_application(owner, vec![element]);
            let method = self.record_method_application(function, owner_application);
            let encoding = hir::ElementEncoding {
                element,
                implementation: hir::InterfaceImplementation {
                    interface: encodable,
                    methods: vec![hir::InterfaceMethodImplementation {
                        member: member.member,
                        target: hir::InterfaceImplementationTarget::Method(method),
                    }],
                },
            };
            match owner {
                Owner::Enum(id) => self.enums[id].element_encoding = Some(encoding),
                Owner::Class(id) => self.classes[id].element_encoding = Some(encoding),
                _ => unreachable!("core containers are an enum or a class"),
            }
            self.derived_encoding_methods
                .push((function, owner, encodable));
        }
    }

    pub(super) fn container_encoding_body(&self, owner: Owner, span: Span) -> Option<ast::Block> {
        let helper = match owner {
            Owner::Enum(id) if self.enums[id].element_encoding.is_some() => "encodeOption",
            Owner::Class(id) if self.classes[id].element_encoding.is_some() => "encodeList",
            _ => return None,
        };
        Some(ast::Block {
            statements: vec![syntax::statement(ast::Expr::Call(ast::CallExpr {
                callee: syntax::ident(helper, span),
                type_args: Vec::new(),
                args: vec![
                    ast::CallArgument::positional(ast::Expr::This { span }),
                    ast::CallArgument::positional(syntax::variable("encoder", span)),
                ],
                span,
            }))],
            span,
        })
    }
}
