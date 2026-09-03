//! Structural type-argument binding used by overload applicability.

use super::*;

impl Lowerer {
    pub(super) fn substitute_call_level(&mut self, ty: TypeId, type_args: &[TypeId]) -> TypeId {
        if type_args.is_empty() {
            ty
        } else {
            self.instantiate_ty(ty, type_args)
        }
    }

    /// Run the M3 binding rules without diagnostics. A conflict or unbound
    /// parameter makes the candidate inapplicable.
    pub(super) fn try_infer_type_args(
        &mut self,
        candidate: &Candidate,
        arg_tys: &[Option<TypeId>],
    ) -> Option<Vec<TypeId>> {
        if candidate.initial_bindings.is_empty() {
            return Some(Vec::new());
        }
        let mut bindings = candidate.initial_bindings.clone();
        for (&param, arg) in candidate.params.iter().zip(arg_tys) {
            let Some(arg) = *arg else {
                continue;
            };
            if !self.try_bind(param, arg, &mut bindings) {
                return None;
            }
        }
        bindings.into_iter().collect()
    }

    pub(crate) fn try_bind(
        &mut self,
        param_ty: TypeId,
        arg_ty: TypeId,
        bindings: &mut [Option<TypeId>],
    ) -> bool {
        match (self.types[param_ty].clone(), self.types[arg_ty].clone()) {
            (Type::Param(index), _) => {
                let index = index.into_raw() as usize;
                match bindings[index] {
                    Some(existing) => self.types_equal(existing, arg_ty),
                    None => {
                        bindings[index] = Some(arg_ty);
                        true
                    }
                }
            }
            (Type::Enum(param), Type::Enum(arg)) => {
                let param = self.enum_applications[param].clone();
                let arg = self.enum_applications[arg].clone();
                param.template != arg.template
                    || (param.arguments.len() == arg.arguments.len()
                        && param
                            .arguments
                            .iter()
                            .zip(arg.arguments)
                            .all(|(param, arg)| self.try_bind(*param, arg, bindings)))
            }
            (Type::Struct(param), Type::Struct(arg)) => {
                let param = self.struct_applications[param].clone();
                let arg = self.struct_applications[arg].clone();
                param.template != arg.template
                    || (param.arguments.len() == arg.arguments.len()
                        && param
                            .arguments
                            .iter()
                            .zip(arg.arguments)
                            .all(|(param, arg)| self.try_bind(*param, arg, bindings)))
            }
            (Type::Class(param), Type::Class(arg)) => {
                let param = self.class_applications[param].clone();
                let arg = self.class_applications[arg].clone();
                param.template != arg.template
                    || (param.arguments.len() == arg.arguments.len()
                        && param
                            .arguments
                            .iter()
                            .zip(arg.arguments)
                            .all(|(param, arg)| self.try_bind(*param, arg, bindings)))
            }
            (Type::Interface(param), Type::Interface(arg)) => {
                let param = self.interface_applications[param].clone();
                let arg = self.interface_applications[arg].clone();
                let arg_args = if param.template == arg.template {
                    Some(arg.arguments)
                } else {
                    self.implemented_interface_application(arg.canonical_type, param.template)
                };
                arg_args.is_none_or(|arg_args| {
                    param.arguments.len() == arg_args.len()
                        && param
                            .arguments
                            .iter()
                            .zip(arg_args)
                            .all(|(param, arg)| self.try_bind(*param, arg, bindings))
                })
            }
            (Type::Interface(param), _) => {
                let param = self.interface_applications[param].clone();
                let Some(arg_args) = self.implemented_interface_application(arg_ty, param.template)
                else {
                    return true;
                };
                param.arguments.len() == arg_args.len()
                    && param
                        .arguments
                        .iter()
                        .zip(arg_args)
                        .all(|(param, arg)| self.try_bind(*param, arg, bindings))
            }
            (Type::Ptr(param), Type::Ptr(arg)) => self.try_bind(param, arg, bindings),
            (Type::Tuple(params), Type::Tuple(args)) if params.len() == args.len() => params
                .iter()
                .zip(args)
                .all(|(param, arg)| self.try_bind(*param, arg, bindings)),
            (Type::Function(param_id), Type::Function(arg_id))
            | (Type::FunPtr(param_id), Type::FunPtr(arg_id)) => {
                let param = self.function_types[param_id].clone();
                let arg = self.function_types[arg_id].clone();
                param.is_suspend == arg.is_suspend
                    && param.parameter_types.len() == arg.parameter_types.len()
                    && param
                        .parameter_types
                        .iter()
                        .zip(arg.parameter_types)
                        .all(|(param, arg)| self.try_bind(*param, arg, bindings))
                    && self.try_bind(param.return_type, arg.return_type, bindings)
            }
            _ => true,
        }
    }
}
