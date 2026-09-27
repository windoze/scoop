use super::*;

mod captures;

pub(super) struct PendingFunction {
    pub(super) name: String,
    pub(super) is_suspend: bool,
    pub(super) modifiers: export::CallableModifiers,
    pub(super) params: Vec<concrete::Param>,
    pub(super) capture_parameters: Vec<concrete::LocalCaptureParameter>,
    pub(super) return_ty: concrete::TypeId,
    pub(super) attributes: export::FunctionAttributes,
    pub(super) kind: concrete::FunctionKind,
    pub(super) receiver: concrete::FunctionReceiver,
    pub(super) span: scoop_ast::Span,
}

impl PendingFunction {
    pub(super) fn finish(
        self,
        materialization: concrete::CallableMaterialization,
    ) -> concrete::Function {
        concrete::Function {
            name: self.name,
            materialization,
            is_suspend: self.is_suspend,
            modifiers: self.modifiers,
            params: self.params,
            capture_parameters: self.capture_parameters,
            return_ty: self.return_ty,
            attributes: self.attributes,
            kind: self.kind,
            receiver: self.receiver,
            span: self.span,
        }
    }
}

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
        let (parameter_count, emittable) = match source {
            FunctionSource::Local(source) => (
                self.source.functions[source].type_param_count(),
                self.is_emittable_source_function(source),
            ),
            FunctionSource::Imported(source) => (
                self.source.imported_generic_templates[source]
                    .type_parameters
                    .len(),
                true,
            ),
        };
        assert_eq!(parameter_count, arguments.len());
        let raw = self.function_slots.len() as u32;
        self.function_slots.push(None);
        self.function_keys.push(key.clone());
        let id = concrete::FunctionId::from_raw(raw.into());
        self.function_by_key.insert(key.clone(), id);
        self.pending_functions.push_back((key, id));
        if emittable {
            self.emitted_functions.push(id);
        }
        id
    }

    pub(super) fn lower_function(&mut self, key: &FunctionKey) -> PendingFunction {
        let source_id = match key.source() {
            FunctionSource::Local(source) => source,
            FunctionSource::Imported(source) => {
                return self.lower_imported_function(source, &self.function_key_arguments(key));
            }
        };
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
        let params: Vec<concrete::Param> = source
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
        let receiver = match method {
            Some(method) => concrete::FunctionReceiver::Method(method),
            None if self.source_function_has_extension_receiver(source_id) => {
                let receiver = params
                    .first()
                    .expect("a source extension has one physical receiver parameter")
                    .ty;
                concrete::FunctionReceiver::Extension(receiver)
            }
            None => concrete::FunctionReceiver::None,
        };
        let capture_parameters = self.local_capture_parameters(source_id, &params);
        PendingFunction {
            name: source.name,
            is_suspend: source.is_suspend,
            modifiers: source.modifiers,
            params,
            capture_parameters,
            return_ty,
            attributes: source.attributes,
            kind,
            receiver,
            span: source.span,
        }
    }

    fn source_function_has_extension_receiver(&self, source: export::FunctionId) -> bool {
        match &self.source.function_identities[source] {
            export::HirFunctionIdentity::Source(identity) => identity
                .declaration()
                .duplicate_signature()
                .receiver_is_present(),
            export::HirFunctionIdentity::PropertyAccessor(accessor) => {
                let property = match accessor {
                    export::HirPropertyAccessorFunction::Getter(getter) => {
                        self.source.property_accessor_identities.get_getter(*getter)
                    }
                    export::HirPropertyAccessorFunction::Setter(setter) => {
                        self.source.property_accessor_identities.get_setter(*setter)
                    }
                }
                .expect("a concrete property accessor has one persistent identity")
                .property();
                self.source.property_identities[property]
                    .extension_id()
                    .is_some()
            }
            export::HirFunctionIdentity::LexicalGenerated(_)
            | export::HirFunctionIdentity::Initialization { .. }
            | export::HirFunctionIdentity::DerivedEquality(_) => false,
        }
    }

    pub(super) fn function_key_arguments(&self, key: &FunctionKey) -> Vec<concrete::TypeId> {
        match key {
            FunctionKey::Free { arguments, .. } | FunctionKey::Imported { arguments, .. } => {
                arguments.clone()
            }
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
}
