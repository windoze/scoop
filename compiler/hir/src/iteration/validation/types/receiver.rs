use std::collections::HashSet;

use super::*;

impl Validator<'_> {
    pub(in super::super) fn validate_receiver_setup(
        &self,
        setup: &[Statement],
        source: BindingTemporary,
        occupied: &mut HashSet<u32>,
        role: &'static str,
    ) -> Check<HashSet<u32>> {
        let mut derived = HashSet::from([source.local.into_raw().into_u32()]);
        for statement in setup {
            let StatementKind::ValDecl {
                pattern: Pattern::Binding { local },
                init,
            } = &statement.kind
            else {
                return fail("zero-argument protocol setup contains a non-binding statement");
            };
            let target = checked_arena(self.locals, *local)
                .ok_or_else(|| invalid("protocol setup binds an invalid local"))?;
            let identity = local.into_raw().into_u32();
            if target.mutable
                || !arena_contains(&self.module.types, target.ty)
                || !arena_contains(&self.module.types, init.ty)
                || target.ty != init.ty
                || !self.validate_receiver_derivation(init, &derived)?
                || occupied.contains(&identity)
                || !derived.insert(identity)
            {
                return fail(role);
            }
            occupied.insert(identity);
        }
        Ok(derived)
    }

    pub(in super::super) fn validate_protocol_call(
        &self,
        call: &Expr,
        derived: &HashSet<u32>,
        operator: OperatorKind,
        role: &'static str,
    ) -> Check {
        let (receiver_ty, info) = match &call.kind {
            ExprKind::MethodCall {
                receiver,
                callee,
                args,
            } => {
                if !args.is_empty() || !self.validate_receiver_derivation(receiver, derived)? {
                    return fail(role);
                }
                let info = self.method_callee_info(*callee)?;
                if checked_arena(&self.module.functions, info.function)
                    .is_none_or(|function| function.method.is_none())
                {
                    return fail(role);
                }
                (receiver.ty, info)
            }
            ExprKind::Call { callee, args } => {
                let [receiver] = args.as_slice() else {
                    return fail(role);
                };
                if !self.validate_receiver_derivation(receiver, derived)? {
                    return fail(role);
                }
                let info = self.callable_info(*callee)?;
                if checked_arena(&self.module.functions, info.function)
                    .is_none_or(|function| function.method.is_some())
                {
                    return fail(role);
                }
                (receiver.ty, info)
            }
            _ => return fail("protocol winner is not an ordinary member or extension call"),
        };
        let function = checked_arena(&self.module.functions, info.function)
            .ok_or_else(|| invalid("protocol call has an invalid function"))?;
        let receiver_matches = match info.receiver {
            Some(ReceiverType::Exact(expected)) if function.method.is_some() => {
                receiver_ty == expected
                    || (!self.is_value_type(receiver_ty)?
                        && !self.is_value_type(expected)?
                        && self.type_is_subtype(receiver_ty, expected)?)
            }
            Some(ReceiverType::Exact(expected)) => receiver_ty == expected,
            Some(ReceiverType::Parameter(expected)) => {
                matches!(self.module.types[receiver_ty], Type::Param(found) if found == expected)
            }
            None => false,
        };
        if info.is_suspend && !self.allows_suspend {
            return fail("suspend iteration protocol call appears in a non-suspend region");
        }
        if function.modifiers
            != (CallableModifiers {
                operator: Some(operator),
                ..CallableModifiers::default()
            })
            || info.parameter_types.len() != 1
            || !receiver_matches
            || info.return_ty != call.ty
            || !arena_contains(&self.module.types, call.ty)
        {
            return fail(role);
        }
        Ok(())
    }

    fn validate_receiver_derivation(
        &self,
        expression: &Expr,
        locals: &HashSet<u32>,
    ) -> Check<bool> {
        if !arena_contains(&self.module.types, expression.ty) {
            return fail("protocol receiver derivation has an invalid type");
        }
        match &expression.kind {
            ExprKind::Local(local) => {
                let Some(declaration) = checked_arena(self.locals, *local) else {
                    return fail("protocol receiver derivation reads an invalid local");
                };
                if !locals.contains(&local.into_raw().into_u32()) {
                    return Ok(false);
                }
                Ok(declaration.ty == expression.ty
                    || (!self.is_value_type(declaration.ty)?
                        && !self.is_value_type(expression.ty)?
                        && self.type_is_subtype(declaration.ty, expression.ty)?))
            }
            ExprKind::Box(value) => Ok(self.validate_receiver_derivation(value, locals)?
                && self.is_value_type(value.ty)?
                && !self.is_value_type(expression.ty)?
                && self.type_is_subtype(value.ty, expression.ty)?),
            ExprKind::FunctionCoercion {
                source,
                coercion,
                target_type,
            } => {
                let Some(coercion) = checked_arena(&self.module.function_coercions, *coercion)
                else {
                    return fail("protocol receiver uses an invalid function coercion");
                };
                let source_type = match checked_arena(&self.module.types, source.ty)
                    .ok_or_else(|| invalid("function coercion source has an invalid type"))?
                {
                    Type::Function(source_type) => *source_type,
                    _ => return Ok(false),
                };
                let result_type = match &self.module.types[expression.ty] {
                    Type::Function(result_type) => *result_type,
                    _ => return Ok(false),
                };
                Ok(self.validate_receiver_derivation(source, locals)?
                    && coercion.source == source_type
                    && coercion.target == *target_type
                    && *target_type == result_type
                    && self.type_is_subtype(source.ty, expression.ty)?)
            }
            _ => Ok(false),
        }
    }

    fn is_value_type(&self, ty: TypeId) -> Check<bool> {
        let ty = checked_arena(&self.module.types, ty)
            .ok_or_else(|| invalid("protocol receiver has an invalid value type"))?;
        Ok(match ty {
            Type::Unit
            | Type::Integer(_)
            | Type::Boolean
            | Type::Struct(_)
            | Type::Enum(_)
            | Type::Tuple(_)
            | Type::Ptr(_)
            | Type::FunPtr(_) => true,
            Type::Param(parameter) => self.parameter(*parameter)?.kind() != TypeParamKind::Ref,
            Type::Any | Type::String | Type::Class(_) | Type::Interface(_) | Type::Function(_) => {
                false
            }
        })
    }

    fn type_is_subtype(&self, source: TypeId, target: TypeId) -> Check<bool> {
        self.type_is_subtype_inner(source, target, &mut HashSet::new())
    }

    fn type_is_subtype_inner(
        &self,
        source: TypeId,
        target: TypeId,
        visiting: &mut HashSet<(u32, u32)>,
    ) -> Check<bool> {
        let source_value = checked_arena(&self.module.types, source)
            .ok_or_else(|| invalid("protocol receiver subtype source is invalid"))?;
        let target_value = checked_arena(&self.module.types, target)
            .ok_or_else(|| invalid("protocol receiver subtype target is invalid"))?;
        if source == target {
            return Ok(true);
        }
        if matches!(target_value, Type::Any) {
            return Ok(true);
        }
        let key = (source.into_raw().into_u32(), target.into_raw().into_u32());
        if !visiting.insert(key) {
            return Ok(false);
        }

        let result = if let Type::Interface(target_application) = target_value {
            let target_application = self.checked_interface_application(*target_application)?;
            self.exact_interface_applications(source, target_application.template)?
                .into_iter()
                .any(|application| {
                    self.module.interface_applications[application].canonical_type == target
                })
        } else {
            match source_value {
                Type::Param(parameter) => {
                    let parameter = self.parameter(*parameter)?;
                    let class = match parameter.class_bound() {
                        Some(bound) => Some(
                            self.checked_class_application(bound.application)?
                                .canonical_type,
                        ),
                        None => None,
                    };
                    match class {
                        Some(class) => self.type_is_subtype_inner(class, target, visiting)?,
                        None => false,
                    }
                }
                Type::Class(application) if matches!(target_value, Type::Class(_)) => {
                    let application = self.checked_class_application(*application)?;
                    let declaration = &self.module.classes[application.template];
                    let Some(base) = declaration.base_class else {
                        visiting.remove(&key);
                        return Ok(false);
                    };
                    let bindings = declaration
                        .type_params
                        .iter()
                        .zip(&application.arguments)
                        .map(|(parameter, argument)| (parameter.id, *argument))
                        .collect::<Vec<_>>();
                    let base = self.instantiate_type(base, &bindings)?;
                    self.type_is_subtype_inner(base, target, visiting)?
                }
                Type::Function(source_function) if matches!(target_value, Type::Function(_)) => {
                    let Type::Function(target_function) = target_value else {
                        unreachable!()
                    };
                    let source_function = self.checked_function_type(*source_function)?;
                    let target_function = self.checked_function_type(*target_function)?;
                    let mut compatible = source_function.is_suspend == target_function.is_suspend
                        && source_function.parameter_types.len()
                            == target_function.parameter_types.len();
                    for (target_parameter, source_parameter) in target_function
                        .parameter_types
                        .iter()
                        .zip(&source_function.parameter_types)
                    {
                        if compatible
                            && !self.type_is_subtype_inner(
                                *target_parameter,
                                *source_parameter,
                                visiting,
                            )?
                        {
                            compatible = false;
                        }
                    }
                    compatible
                        && self.type_is_subtype_inner(
                            source_function.return_type,
                            target_function.return_type,
                            visiting,
                        )?
                }
                _ => false,
            }
        };
        visiting.remove(&key);
        Ok(result)
    }
}
