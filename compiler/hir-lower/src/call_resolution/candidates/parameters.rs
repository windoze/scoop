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
        let complete = state.complete_parameter_substitution(
            self.owner_parameters
                .iter()
                .chain(&self.callable_parameters),
            bindings,
        );
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
            state.substitute_type_parameter(parameter, bindings);
        }
        self.return_type = state.instantiate_method_ty(self.return_type, bindings);
    }
}

impl crate::Lowerer {
    pub(crate) fn complete_parameter_substitution<'a>(
        &mut self,
        parameters: impl Iterator<Item = &'a hir::TypeParamDecl>,
        bindings: &[(hir::TypeParamId, hir::TypeId)],
    ) -> Vec<(hir::TypeParamId, hir::TypeId)> {
        let mut complete = bindings.to_vec();
        for parameter in parameters {
            if !complete.iter().any(|(id, _)| *id == parameter.id) {
                complete.push((
                    parameter.id,
                    self.intern_type(hir::Type::Param(parameter.id)),
                ));
            }
        }
        complete
    }

    pub(crate) fn substitute_type_parameter(
        &mut self,
        parameter: &mut hir::TypeParamDecl,
        bindings: &[(hir::TypeParamId, hir::TypeId)],
    ) {
        for requirement in &mut parameter.requirements {
            requirement
                .contract
                .map_types(|ty| self.instantiate_method_ty(ty, bindings));
        }
        for (positive, excluded) in [(&mut parameter.bounds, &mut parameter.excluded_interfaces)]
            .into_iter()
            .chain(
                parameter
                    .companion
                    .as_mut()
                    .map(|bounds| (&mut bounds.bounds, &mut bounds.excluded_interfaces)),
            )
        {
            if let hir::TypeParamBounds::Nominal(bounds) = positive {
                if let Some(bound) = &mut bounds.class {
                    bound.ty = self.instantiate_method_ty(bound.ty, bindings);
                }
                for bound in &mut bounds.interfaces {
                    bound.ty = self.instantiate_method_ty(bound.ty, bindings);
                }
            }
            for bound in excluded {
                bound.ty = self.instantiate_method_ty(bound.ty, bindings);
            }
        }
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
