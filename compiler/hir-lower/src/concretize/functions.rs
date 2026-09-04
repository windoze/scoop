use super::*;

impl Concretizer<'_> {
    pub(super) fn is_emittable_source_function(&self, id: export::FunctionId) -> bool {
        let function = &self.source.functions[id];
        if !matches!(
            function.kind,
            export::FunctionKind::User(_) | export::FunctionKind::DerivedEquality
        ) {
            return false;
        }
        let Some(method) = function.method else {
            return true;
        };
        !matches!(
            self.source.types[method.owner],
            export::Type::Interface(..) | export::Type::Any
        )
    }

    pub(super) fn request_function(
        &mut self,
        source: export::FunctionId,
        arguments: Vec<concrete::TypeId>,
    ) -> concrete::FunctionId {
        assert!(
            self.source.functions[source].method.is_none(),
            "method instances require an exact concrete owner"
        );
        let key = FunctionKey::Free { source, arguments };
        self.request_function_key(key)
    }

    pub(super) fn request_method(
        &mut self,
        source: export::FunctionId,
        owner: concrete::MethodOwner,
        specialization: MethodRequest,
    ) -> concrete::FunctionId {
        assert!(
            self.source.functions[source].method.is_some(),
            "method requests name a method declaration"
        );
        self.request_function_key(FunctionKey::Method {
            source,
            owner,
            specialization,
        })
    }

    pub(super) fn request_function_key(&mut self, key: FunctionKey) -> concrete::FunctionId {
        if let Some(&id) = self.function_by_key.get(&key) {
            return id;
        }
        let source = key.source();
        let arguments = self.function_key_arguments(&key);
        assert_eq!(
            self.source.functions[source].type_param_count(),
            arguments.len()
        );
        let raw = self.function_slots.len() as u32;
        self.function_slots.push(None);
        let id = concrete::FunctionId::from_raw(raw.into());
        self.function_by_key.insert(key.clone(), id);
        self.pending_functions.push_back((key, id));
        if self.is_emittable_source_function(source) {
            self.emitted_functions.push(id);
        }
        id
    }

    pub(super) fn lower_function(&mut self, key: &FunctionKey) -> concrete::Function {
        let source_id = key.source();
        let source = self.source.functions[source_id].clone();
        let arguments = self.function_key_arguments(key);
        let (kind, local_map) = match &source.kind {
            export::FunctionKind::User(body) => {
                let (body, local_map) = self.lower_body(body, &arguments);
                (concrete::FunctionKind::User(body), local_map)
            }
            export::FunctionKind::DerivedEquality => {
                let (body, local_map) = self.derived_bodies.get(key).cloned().expect(
                    "a typed derived application supplies its concrete body before emission",
                );
                (concrete::FunctionKind::User(body), local_map)
            }
            export::FunctionKind::Intrinsic(intrinsic) => {
                (concrete::FunctionKind::Intrinsic(*intrinsic), Vec::new())
            }
            export::FunctionKind::Extern(id) => (
                concrete::FunctionKind::Extern(self.extern_map[id]),
                Vec::new(),
            ),
        };
        let params = source
            .params
            .iter()
            .map(|param| concrete::Param {
                name: param.name.clone(),
                ty: self.lower_type(param.ty, &arguments),
                local: local_map
                    .get(param.local.into_raw().into_u32() as usize)
                    .copied()
                    .unwrap_or_else(|| remap_idx(param.local)),
            })
            .collect();
        let return_ty = self.lower_type(source.return_ty, &arguments);
        let method = source.method.map(|method| concrete::Method {
            owner: self.lower_type(method.owner, &arguments),
            modifier: method.modifier,
            dispatch: self.lower_method_dispatch(method.dispatch, key),
        });
        let origin = self.function_origin(key, &source);
        concrete::Function {
            name: source.name,
            origin,
            is_suspend: source.is_suspend,
            modifiers: source.modifiers,
            params,
            return_ty,
            attributes: source.attributes,
            kind,
            method,
            span: source.span,
        }
    }

    pub(super) fn function_key_arguments(&self, key: &FunctionKey) -> Vec<concrete::TypeId> {
        match key {
            FunctionKey::Free { arguments, .. } => arguments.clone(),
            FunctionKey::Method {
                owner,
                specialization,
                ..
            } => {
                let mut arguments = self.concrete_method_owner_arguments(*owner).to_vec();
                if let MethodRequest::Generic {
                    method_arguments, ..
                } = specialization
                {
                    arguments.extend(method_arguments.iter().copied());
                }
                arguments
            }
        }
    }

    pub(super) fn concrete_method_owner_arguments(
        &self,
        owner: concrete::MethodOwner,
    ) -> &[concrete::TypeId] {
        match owner {
            concrete::MethodOwner::Class(id) => &self.classes[id].type_arguments,
            concrete::MethodOwner::Struct(id) => &self.structs[id].type_arguments,
            concrete::MethodOwner::Enum(id) => &self.enums[id].type_arguments,
            concrete::MethodOwner::Interface(id) => &self.interfaces[id].type_arguments,
            concrete::MethodOwner::Object(_) => &[],
            concrete::MethodOwner::Structural(_) => &[],
        }
    }

    pub(super) fn concrete_function_arguments(
        &self,
        function: &concrete::Function,
    ) -> Vec<concrete::TypeId> {
        match &function.origin {
            concrete::FunctionOrigin::Free(concrete::FreeFunctionOrigin::Plain) => Vec::new(),
            concrete::FunctionOrigin::Free(concrete::FreeFunctionOrigin::Generic {
                arguments,
                ..
            }) => arguments.to_vec(),
            concrete::FunctionOrigin::Method(origin) => {
                let mut arguments = self.concrete_method_owner_arguments(origin.owner).to_vec();
                if let concrete::MethodSpecialization::Generic {
                    method_arguments, ..
                } = &origin.specialization
                {
                    arguments.extend(method_arguments.iter().copied());
                }
                arguments
            }
        }
    }

    pub(super) fn function_origin(
        &self,
        key: &FunctionKey,
        source: &export::Function,
    ) -> concrete::FunctionOrigin {
        match key {
            FunctionKey::Free {
                source: source_id,
                arguments,
            } => match source.genericity {
                export::FunctionGenericity::Plain => {
                    concrete::FunctionOrigin::Free(concrete::FreeFunctionOrigin::Plain)
                }
                export::FunctionGenericity::Generic { definition, .. } => {
                    concrete::FunctionOrigin::Free(concrete::FreeFunctionOrigin::Generic {
                        origin: concrete::GenericFunctionOriginId::from_raw(
                            definition.into_raw().into_u32(),
                        ),
                        arguments: concrete::NonEmptyVec::from_vec(arguments.clone())
                            .expect("a generic function application has non-empty arguments"),
                        symbol: self.instance_symbol(&source.name, source_id.into_raw().into_u32()),
                    })
                }
                export::FunctionGenericity::OwnerParameterizedMethod { .. }
                | export::FunctionGenericity::GenericMethod { .. } => {
                    unreachable!("free function keys cannot name methods")
                }
            },
            FunctionKey::Method {
                source: source_id,
                owner,
                specialization,
            } => {
                let specialization = match specialization {
                    MethodRequest::Plain => match source.genericity {
                        export::FunctionGenericity::Plain => concrete::MethodSpecialization::Plain,
                        export::FunctionGenericity::OwnerParameterizedMethod { .. } => {
                            concrete::MethodSpecialization::OwnerParameterized {
                                origin: concrete::OwnerParameterizedMethodOriginId::from_raw(
                                    source_id.into_raw().into_u32(),
                                ),
                                symbol: self
                                    .instance_symbol(&source.name, source_id.into_raw().into_u32()),
                            }
                        }
                        export::FunctionGenericity::Generic { .. }
                        | export::FunctionGenericity::GenericMethod { .. } => {
                            unreachable!("plain method requests match plain method declarations")
                        }
                    },
                    MethodRequest::Generic {
                        definition,
                        method_arguments,
                    } => concrete::MethodSpecialization::Generic {
                        origin: concrete::GenericMethodOriginId::from_raw(
                            definition.into_raw().into_u32(),
                        ),
                        method_arguments: concrete::NonEmptyVec::from_vec(
                            method_arguments.to_vec(),
                        )
                        .expect("a generic method request has non-empty method arguments"),
                        symbol: self.instance_symbol(&source.name, source_id.into_raw().into_u32()),
                    },
                };
                concrete::FunctionOrigin::Method(concrete::MethodOrigin {
                    owner: *owner,
                    specialization,
                })
            }
        }
    }

    pub(super) fn instance_symbol(
        &self,
        name: &str,
        discriminator: u32,
    ) -> concrete::InstanceSymbol {
        if self.overloaded_generic_names.contains(name) {
            concrete::InstanceSymbol::Overloaded { discriminator }
        } else {
            concrete::InstanceSymbol::Unique
        }
    }
}
