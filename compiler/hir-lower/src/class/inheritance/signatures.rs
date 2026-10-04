use super::*;
use crate::{FnParamCalling, FnVarargOmission};

impl Lowerer {
    /// Whether the method is `abstract` (bodyless class method).
    pub(super) fn is_abstract_method(&self, id: FunctionId) -> bool {
        self.functions[id]
            .method
            .is_some_and(|method| method.modifier == hir::MethodModifier::Abstract)
    }

    pub(in crate::class) fn instantiated_signature(
        &mut self,
        method: FunctionId,
        owner_arguments: &[TypeId],
        target_method_parameters: &[hir::TypeParamDecl],
    ) -> FnSig {
        let sig = self.signatures[&method].clone();
        let (owner_parameters, method_parameters) = match &self.functions[method].genericity {
            hir::FunctionGenericity::Plain => (Vec::new(), Vec::new()),
            hir::FunctionGenericity::OwnerParameterizedMethod {
                owner_parameters, ..
            } => (owner_parameters.clone(), Vec::new()),
            hir::FunctionGenericity::GenericMethod {
                owner_parameters,
                method_parameters,
                ..
            } => (
                owner_parameters.clone(),
                method_parameters.iter().cloned().collect(),
            ),
            hir::FunctionGenericity::Generic { .. } => {
                unreachable!("nominal methods do not use generic-function identity")
            }
        };
        assert_eq!(owner_parameters.len(), owner_arguments.len());
        assert_eq!(method_parameters.len(), target_method_parameters.len());
        let mut bindings = owner_parameters
            .iter()
            .zip(owner_arguments.iter().copied())
            .map(|(parameter, argument)| (parameter.id, argument))
            .collect::<Vec<_>>();
        let target_method_types = target_method_parameters
            .iter()
            .map(|parameter| (parameter.id, self.intern_type(Type::Param(parameter.id))))
            .collect::<Vec<_>>();
        bindings.extend(
            method_parameters
                .iter()
                .zip(&target_method_types)
                .map(|(source, (_, target))| (source.id, *target)),
        );
        FnSig {
            context_parameters: sig
                .context_parameters
                .into_iter()
                .map(|parameter| hir::ContextParameter {
                    ty: self.instantiate_method_ty(parameter.ty, &bindings),
                    ..parameter
                })
                .collect(),
            is_suspend: sig.is_suspend,
            modifiers: sig.modifiers,
            attributes: sig.attributes,
            owner_type_param_count: 0,
            type_params: target_method_parameters.to_vec(),
            params: sig
                .params
                .into_iter()
                .map(|param| {
                    let calling = match param.calling {
                        FnParamCalling::Required => FnParamCalling::Required,
                        FnParamCalling::Default { expression } => {
                            FnParamCalling::Default { expression }
                        }
                        FnParamCalling::Vararg {
                            element_ty,
                            omission,
                        } => FnParamCalling::Vararg {
                            element_ty: self.instantiate_method_ty(element_ty, &bindings),
                            omission: match omission {
                                FnVarargOmission::EmptyArray => FnVarargOmission::EmptyArray,
                                FnVarargOmission::Default { expression } => {
                                    FnVarargOmission::Default { expression }
                                }
                            },
                        },
                    };
                    FnParam {
                        name: param.name,
                        calling,
                        ty: self.instantiate_method_ty(param.ty, &bindings),
                    }
                })
                .collect(),
            return_ty: self.instantiate_method_ty(sig.return_ty, &bindings),
        }
    }

    pub(super) fn same_instantiated_signature(
        &mut self,
        candidate: FunctionId,
        name: &str,
        sig: &FnSig,
        args: &[TypeId],
    ) -> bool {
        self.same_instantiated_signature_shape(candidate, name, sig, args)
            && self.functions[candidate].is_suspend == sig.is_suspend
            && self.functions[candidate].modifiers == sig.modifiers
    }

    pub(super) fn same_instantiated_signature_shape(
        &mut self,
        candidate: FunctionId,
        name: &str,
        sig: &FnSig,
        args: &[TypeId],
    ) -> bool {
        let target_method_parameters = &sig.type_params[sig.owner_type_param_count..];
        if self.functions[candidate].method_type_param_count() != target_method_parameters.len() {
            return false;
        }
        let candidate_sig = self.instantiated_signature(candidate, args, target_method_parameters);
        let candidate_own_count =
            candidate_sig.type_params.len() - candidate_sig.owner_type_param_count;
        let expected_own_count = sig.type_params.len() - sig.owner_type_param_count;
        self.functions[candidate].name.rsplit('.').next() == Some(name)
            && candidate_own_count == expected_own_count
            && candidate_sig.params.len() == sig.params.len()
            && candidate_sig
                .params
                .iter()
                .zip(&sig.params)
                .all(|(a, b)| self.types_equal(a.ty, b.ty))
            && self.types_equal(candidate_sig.return_ty, sig.return_ty)
    }
}
