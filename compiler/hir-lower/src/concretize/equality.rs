use super::*;

impl Concretizer<'_> {
    pub(super) fn lower_derived_equality_application(
        &mut self,
        source: export::DerivedEqualityApplicationId,
        substitution: &[concrete::TypeId],
    ) -> concrete::Callable {
        let application = self.source.derived_equality_applications[source].clone();
        let owner_ty = self.lower_type(application.owner_ty, substitution);
        if let Some(&function) = self.derived_functions.get(&owner_ty) {
            return concrete::Callable::Function(function);
        }
        match application.origin {
            export::DerivedEqualityOrigin::Nominal(owner) => {
                let owner = self.lower_method_owner(owner, substitution);
                let source = FunctionSource::Local(application.function);
                let key = self.function_key(
                    source,
                    Some(owner),
                    self.concrete_method_owner_arguments(owner).to_vec(),
                );
                let function = self.request_function_key(key.clone(), source);
                self.derived_functions.insert(owner_ty, function);
                let body = self.lower_body(&application.body, substitution);
                self.derived_bodies.insert(key, body);
                concrete::Callable::Function(function)
            }
            export::DerivedEqualityOrigin::TypeOwned(source_owner_ty) => {
                debug_assert_eq!(source_owner_ty, application.owner_ty);
                // Reserve and publish the identity before lowering the body,
                // so nested structural applications can refer back to it
                // without requiring a downstream recursion heuristic.
                let raw = self.function_slots.len() as u32;
                self.function_slots.push(None);
                let location = FunctionSource::Local(application.function);
                let key = self.function_key(
                    location,
                    Some(concrete::MethodOwner::TypeOwned(owner_ty)),
                    Vec::new(),
                );
                self.function_keys.push(key.clone());
                self.function_sources.push(location);
                let function = concrete::FunctionId::from_raw(raw.into());
                self.function_by_key.insert(key, function);
                self.derived_functions.insert(owner_ty, function);

                let (body, local_map) = self.lower_body(&application.body, substitution);
                let source_function = &self.source.functions[application.function];
                let params = source_function
                    .params
                    .iter()
                    .map(|param| concrete::Param {
                        name: param.name.clone(),
                        ty: owner_ty,
                        local: local_map[param.local.into_raw().into_u32() as usize],
                    })
                    .collect();
                let value = PendingFunction {
                    name: format!("$derived.equals.{}", source.into_raw().into_u32()),
                    is_suspend: false,
                    modifiers: export::CallableModifiers {
                        operator: Some(export::OperatorKind::Equals),
                        property_delegate_operator: None,
                        is_infix: false,
                    },
                    params,
                    capture_parameters: Vec::new(),
                    return_ty: self.lower_type(source_function.return_ty, substitution),
                    attributes: application.attributes,
                    kind: concrete::FunctionKind::User(body),
                    receiver: concrete::FunctionReceiver::Method(concrete::Method {
                        owner: owner_ty,
                        modifier: concrete::MethodModifier::Final,
                        dispatch: concrete::MethodDispatch::Direct,
                    }),
                    span: application.span,
                };
                let slot = function.into_raw().into_u32() as usize;
                assert!(self.function_slots[slot].replace(value).is_none());
                self.emitted_functions.push(function);
                concrete::Callable::Function(function)
            }
        }
    }

    pub(super) fn lower_public_equality(&mut self, owner: export::TypeId) {
        if let Some((application, _)) = self
            .source
            .derived_equality_applications
            .iter()
            .find(|(_, application)| application.owner_ty == owner)
        {
            self.lower_derived_equality_application(application, &[]);
        }
    }
}
