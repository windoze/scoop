//! Declaration parameter types and source calling protocols share one view.

use crate::defaults::DefaultArgumentSource;
use scoop_hir as hir;

#[derive(Debug, Clone)]
pub(crate) struct DeclarationSignature<D = DefaultArgumentSource> {
    pub(crate) owner_parameters: Vec<hir::TypeParamDecl>,
    pub(crate) callable_parameters: Vec<hir::TypeParamDecl>,
    pub(crate) value_parameters: Vec<ValueParameter<D>>,
    pub(crate) return_type: hir::TypeId,
}

impl<D> DeclarationSignature<D> {
    pub(crate) fn substitute_types(
        &mut self,
        state: &mut crate::Lowerer,
        bindings: &[(hir::TypeParamId, hir::TypeId)],
    ) {
        // Parameters that remain declared retain their identity; only the
        // explicit relation replaces captured owners or selected arguments.
        let mut complete = bindings.to_vec();
        for parameter in self
            .owner_parameters
            .iter()
            .chain(&self.callable_parameters)
        {
            if !complete.iter().any(|(id, _)| *id == parameter.id) {
                complete.push((
                    parameter.id,
                    state.intern_type(hir::Type::Param(parameter.id)),
                ));
            }
        }
        let bindings = &complete;
        for parameter in &mut self.value_parameters {
            parameter.ty = state.instantiate_method_ty(parameter.ty, bindings);
            if let ValueParameterCalling::Vararg {
                element_type,
                array_type,
                ..
            } = &mut parameter.calling
            {
                *element_type = state.instantiate_method_ty(*element_type, bindings);
                *array_type = state.instantiate_method_ty(*array_type, bindings);
            }
        }
        for parameter in self
            .owner_parameters
            .iter_mut()
            .chain(&mut self.callable_parameters)
        {
            if let hir::TypeParamBounds::Nominal(bounds) = &mut parameter.bounds {
                if let Some(bound) = &mut bounds.class {
                    bound.ty = state.instantiate_method_ty(bound.ty, bindings);
                }
                for bound in &mut bounds.interfaces {
                    bound.ty = state.instantiate_method_ty(bound.ty, bindings);
                }
            }
            for bound in &mut parameter.excluded_interfaces {
                bound.ty = state.instantiate_method_ty(bound.ty, bindings);
            }
        }
        self.return_type = state.instantiate_method_ty(self.return_type, bindings);
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ValueParameter<D = DefaultArgumentSource> {
    pub(crate) name: String,
    pub(crate) calling: ValueParameterCalling<D>,
    pub(crate) ty: hir::TypeId,
}

impl<D> ValueParameter<D> {
    pub(crate) fn is_vararg(&self) -> bool {
        matches!(self.calling, ValueParameterCalling::Vararg { .. })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValueParameterCalling<D = DefaultArgumentSource> {
    Required,
    Default(D),
    Vararg {
        element_type: hir::TypeId,
        array_type: hir::TypeId,
        omission: VarargOmission<D>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VarargOmission<D = DefaultArgumentSource> {
    EmptyArray,
    Default(D),
}
