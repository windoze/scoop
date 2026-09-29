//! Shared projections used by several expression families.

use crate::{
    DefaultArrayAssemblyPartV1, DefaultArrayAssemblyV1, DefaultMethodCalleeV1, DefaultPlaceV1,
    OptionalDefaultExpressionV1, Place,
};

use super::super::BodyProjection;

impl BodyProjection<'_, '_> {
    pub(in crate::production::default_templates::body) fn imported_method_callee(
        &self,
        callee: &crate::ImportedMethodCallee,
    ) -> Result<DefaultMethodCalleeV1, super::super::super::DefaultBodyProjectionError> {
        Ok(match callee {
            crate::ImportedMethodCallee::Callable(callable) => {
                DefaultMethodCalleeV1::Callable(self.imported_reference_callee(*callable)?)
            }
            crate::ImportedMethodCallee::InterfaceBound(bound) => DefaultMethodCalleeV1::Bound(
                self.entities.imported_bound_callable(bound, self.binders)?,
            ),
            crate::ImportedMethodCallee::DerivedEquality(application) => {
                self.method_callee(crate::MethodCallee::DerivedEquality(*application))?
            }
        })
    }

    pub(super) fn optional_expression(
        &mut self,
        expression: Option<&crate::Expr>,
    ) -> Result<OptionalDefaultExpressionV1, super::super::super::DefaultBodyProjectionError> {
        expression
            .map(|expression| {
                self.expression(expression)
                    .map(OptionalDefaultExpressionV1::present)
            })
            .transpose()
            .map(|expression| expression.unwrap_or_else(OptionalDefaultExpressionV1::absent))
    }

    pub(super) fn place(
        &self,
        place: Place,
    ) -> Result<DefaultPlaceV1, super::super::super::DefaultBodyProjectionError> {
        Ok(match place {
            Place::Local(local) => DefaultPlaceV1::Local {
                local: self.local(local)?,
            },
            Place::Global(global) => DefaultPlaceV1::Global {
                property: self.entities.global_property(global)?,
            },
        })
    }

    pub(in crate::production::default_templates::body) fn method_callee(
        &self,
        callee: crate::MethodCallee,
    ) -> Result<DefaultMethodCalleeV1, super::super::super::DefaultBodyProjectionError> {
        Ok(match callee {
            crate::MethodCallee::Callable(callable) => {
                DefaultMethodCalleeV1::Callable(self.entities.callable(callable, self.binders)?)
            }
            crate::MethodCallee::Bound(bound) => {
                DefaultMethodCalleeV1::Bound(self.entities.bound_callable(bound, self.binders)?)
            }
            crate::MethodCallee::DerivedEquality(application_id) => {
                let application = super::super::super::arena_get(
                    &self.entities.export().derived_equality_applications,
                    application_id,
                )
                .ok_or_else(|| self.unknown("derived equality application", application_id))?;
                DefaultMethodCalleeV1::DerivedEquality {
                    owner_type: self.type_key(application.owner_ty)?,
                }
            }
        })
    }

    pub(super) fn array_assembly(
        &mut self,
        assembly: &crate::ArrayAssembly,
    ) -> Result<DefaultArrayAssemblyV1, super::super::super::DefaultBodyProjectionError> {
        let mut parts = Vec::with_capacity(assembly.parts.len());
        for part in &assembly.parts {
            parts.push(match part {
                crate::ArrayAssemblyPart::Element(value) => {
                    DefaultArrayAssemblyPartV1::Element(self.expression(value)?)
                }
                crate::ArrayAssemblyPart::CopyArray(value) => {
                    DefaultArrayAssemblyPartV1::CopyArray(self.expression(value)?)
                }
            });
        }
        let result = super::super::super::arena_get(
            &self.entities.export().class_applications,
            assembly.result_type,
        )
        .ok_or_else(|| self.unknown("array result application", assembly.result_type))?;
        DefaultArrayAssemblyV1::try_new(
            self.type_key(assembly.element_type)?,
            parts,
            self.type_key(result.canonical_type)?,
        )
        .map_err(super::super::super::DefaultBodyProjectionError::ArrayAssembly)
    }

    pub(super) fn struct_application_type(
        &self,
        application_id: crate::StructApplicationId,
    ) -> Result<scoop_identity::SignatureTypeKey, super::super::super::DefaultBodyProjectionError>
    {
        let application = super::super::super::arena_get(
            &self.entities.export().struct_applications,
            application_id,
        )
        .ok_or_else(|| self.unknown("struct application", application_id))?;
        self.type_key(application.canonical_type)
    }

    pub(super) fn unknown<T>(
        &self,
        kind: &'static str,
        id: la_arena::Idx<T>,
    ) -> super::super::super::DefaultBodyProjectionError {
        super::super::super::DefaultEntityProjectionError::Unknown {
            kind,
            index: super::super::super::raw_index(id),
        }
        .into()
    }
}
