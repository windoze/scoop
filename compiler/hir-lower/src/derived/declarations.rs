use super::*;

impl Lowerer {
    pub(crate) fn declare_derived_equality_methods(&mut self) {
        let structs = self.structs.iter().map(|(id, _)| id).collect::<Vec<_>>();
        for id in structs {
            if matches!(
                self.structs[id].representation,
                hir::StructRepresentation::Intrinsic(_)
            ) {
                continue;
            }
            let owner = Owner::Struct(id);
            if self.has_same_type_equals(owner) {
                continue;
            }
            let function = self.declare_derived_equality_method(owner);
            self.structs[id].derived_equality = Some(function);
        }

        let enums = self.enums.iter().map(|(id, _)| id).collect::<Vec<_>>();
        for id in enums {
            let owner = Owner::Enum(id);
            if self.has_same_type_equals(owner) {
                continue;
            }
            let function = self.declare_derived_equality_method(owner);
            self.enums[id].derived_equality = Some(function);
        }
    }

    fn has_same_type_equals(&mut self, owner: Owner) -> bool {
        let owner_ty = self.owner_ty(owner);
        let methods = match owner {
            Owner::Struct(id) => &self.structs[id].methods,
            Owner::Enum(id) => &self.enums[id].methods,
            Owner::Class(id) => &self.classes[id].methods,
            Owner::Interface(id) => &self.interface_methods[&id],
            Owner::Object(id) => &self.classes[self.objects[id].backing_class].methods,
        };
        methods.iter().copied().any(|function| {
            let signature = &self.signatures[&function];
            signature.modifiers.operator == Some(hir::OperatorKind::Equals)
                && matches!(signature.params.as_slice(), [parameter] if self.types_equal(parameter.ty, owner_ty))
        })
    }

    fn declare_derived_equality_method(&mut self, owner: Owner) -> hir::FunctionId {
        let owner_ty = self.owner_ty(owner);
        let owner_parameters = self.owner_type_params(owner);
        let span = match owner {
            Owner::Struct(id) => self.structs[id].span,
            Owner::Enum(id) => self.enums[id].span,
            Owner::Class(id) => self.classes[id].span,
            Owner::Interface(id) => self.interfaces[id].span,
            Owner::Object(id) => self.objects[id].span,
        };
        let mut attributes = hir::FunctionAttributes::default();
        if self.requires_unsafe_use(owner_ty) {
            attributes.safety = hir::Safety::Unsafe;
        }
        // Derived declarations do not own a body-local arena. Every requested
        // application supplies one whose first two locals are structurally
        // the receiver and argument, so the declaration can name those slots
        // without consuming unrelated lexical binding identities.
        let this = hir::LocalId::from_raw(0.into());
        let other = hir::LocalId::from_raw(1.into());
        let access = self.fixed_representation_access(owner);
        let file = match owner {
            Owner::Struct(id) => self.struct_files[&id],
            Owner::Enum(id) => self.enum_files[&id],
            Owner::Class(id) => self.class_files[&id],
            Owner::Interface(id) => self.interface_files[&id],
            Owner::Object(id) => self.object_files[&id],
        };
        let function = self.functions.alloc(Function {
            signature: hir::CallableSignature {
                name: format!("{}.equals", owner.describe_name(self)),
                is_suspend: false,
                modifiers: hir::CallableModifiers {
                    operator: Some(hir::OperatorKind::Equals),
                    property_delegate_operator: None,
                    is_infix: false,
                },
                params: vec![
                    hir::Param {
                        name: "this".to_string(),
                        ty: owner_ty,
                        local: this,
                    },
                    hir::Param {
                        name: "other".to_string(),
                        ty: owner_ty,
                        local: other,
                    },
                ],
                return_ty: self.boolean,
                attributes,
                span,
            },

            access,
            genericity: hir::FunctionGenericity::Plain,
            kind: FunctionKind::DerivedEquality,
            method: Some(hir::Method {
                owner: owner_ty,
                modifier: hir::MethodModifier::Final,
                dispatch: hir::MethodDispatch::Direct,
            }),
        });
        self.register_method_parameters(function, owner_parameters.clone(), Vec::new());
        self.function_owner.insert(function, owner);
        self.function_files.insert(function, file);
        self.signatures.insert(
            function,
            FnSig {
                is_suspend: false,
                modifiers: hir::CallableModifiers {
                    operator: Some(hir::OperatorKind::Equals),
                    property_delegate_operator: None,
                    is_infix: false,
                },
                attributes,
                owner_type_param_count: owner_parameters.len(),
                type_params: owner_parameters,
                params: vec![FnParam {
                    name: ast::Ident {
                        text: "other".to_string(),
                        span,
                    },
                    calling: FnParamCalling::Required,
                    ty: owner_ty,
                }],
                return_ty: self.boolean,
            },
        );
        function
    }

    pub(super) fn declare_structural_derived_equality_method(
        &mut self,
        owner_ty: hir::TypeId,
        span: ast::Span,
    ) -> hir::FunctionId {
        let mut attributes = hir::FunctionAttributes::default();
        if self.requires_unsafe_use(owner_ty) {
            attributes.safety = hir::Safety::Unsafe;
        }
        let this = hir::LocalId::from_raw(0.into());
        let other = hir::LocalId::from_raw(1.into());
        let access = hir::DeclarationAccess {
            declared: hir::DeclaredVisibility::Public,
            lookup: hir::EffectiveLookupDomain(self.type_access_domain(owner_ty)),
            slot: None,
        };
        let function = self.functions.alloc(Function {
            signature: hir::CallableSignature {
                name: format!("{}.equals", self.type_name(owner_ty)),
                is_suspend: false,
                modifiers: hir::CallableModifiers {
                    operator: Some(hir::OperatorKind::Equals),
                    property_delegate_operator: None,
                    is_infix: false,
                },
                params: vec![
                    hir::Param {
                        name: "this".to_string(),
                        ty: owner_ty,
                        local: this,
                    },
                    hir::Param {
                        name: "other".to_string(),
                        ty: owner_ty,
                        local: other,
                    },
                ],
                return_ty: self.boolean,
                attributes,
                span,
            },

            access,
            genericity: hir::FunctionGenericity::Plain,
            kind: FunctionKind::DerivedEquality,
            method: Some(hir::Method {
                owner: owner_ty,
                modifier: hir::MethodModifier::Final,
                dispatch: hir::MethodDispatch::Direct,
            }),
        });
        self.function_files.insert(function, self.current_file);
        self.signatures.insert(
            function,
            FnSig {
                is_suspend: false,
                modifiers: hir::CallableModifiers {
                    operator: Some(hir::OperatorKind::Equals),
                    property_delegate_operator: None,
                    is_infix: false,
                },
                attributes,
                owner_type_param_count: 0,
                type_params: Vec::new(),
                params: vec![FnParam {
                    name: ast::Ident {
                        text: "other".to_string(),
                        span,
                    },
                    calling: FnParamCalling::Required,
                    ty: owner_ty,
                }],
                return_ty: self.boolean,
            },
        );
        function
    }
}
