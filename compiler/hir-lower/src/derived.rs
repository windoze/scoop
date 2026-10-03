//! Compiler-derived value-type members.
//!
//! A declaration in this module is only stable callable identity and a typed
//! conditional signature. Every requested application later receives a
//! complete ordinary HIR body whose nested calls have already been resolved.

use scoop_ast as ast;
use scoop_hir as hir;

use crate::{
    CallableCandidate, FnParam, FnParamCalling, FnSig, Function, FunctionKind, Lowerer, Owner, Type,
};

mod body;
mod declarations;
mod imported;
mod publication;

pub(crate) enum DerivedEqualityCandidate {
    Imported(hir::ImportedDerivedEquality),
    Nominal {
        overload: CallableCandidate,
        application: hir::DerivedEqualityApplicationId,
    },
    TypeOwned {
        function: hir::FunctionId,
        application: hir::DerivedEqualityApplicationId,
    },
}

impl Lowerer {
    pub(crate) fn derived_equality_candidate_at(
        &mut self,
        ty: hir::TypeId,
        origin: hir::ExpressionOrigin,
    ) -> Result<Option<DerivedEqualityCandidate>, String> {
        let previous = self.derived_expression_origin.replace(origin);
        let result = self.derived_equality_candidate(ty, origin.concrete().definition.span);
        self.derived_expression_origin = previous;
        result
    }

    pub(crate) fn derived_equality_candidate(
        &mut self,
        ty: hir::TypeId,
        span: ast::Span,
    ) -> Result<Option<DerivedEqualityCandidate>, String> {
        let nominal = match self.types[ty].clone() {
            Type::Struct(_) | Type::Enum(_)
                if self.dependency_nominal_application(ty).is_some() =>
            {
                let (_, arguments) = self
                    .dependency_nominal_application(ty)
                    .expect("an imported value retains its nominal application");
                let parameter_free = arguments.is_empty();
                if self.has_imported_same_type_equals(ty)? {
                    return Ok(None);
                }
                if parameter_free {
                    return self.imported_equality_candidate(ty, span);
                }
                None
            }
            Type::Struct(application) => {
                let declaration =
                    &self.structs[self.struct_id(self.struct_applications[application].template)];
                let Some(function) = declaration.derived_equality else {
                    return Ok(None);
                };
                Some((function, hir::MethodOwnerApplication::Struct(application)))
            }
            Type::Enum(application) => {
                let declaration =
                    &self.enums[self.enum_id(self.enum_applications[application].template)];
                let Some(function) = declaration.derived_equality else {
                    return Ok(None);
                };
                Some((function, hir::MethodOwnerApplication::Enum(application)))
            }
            Type::Unit | Type::Tuple(_) => None,
            _ => return Ok(None),
        };
        if let Some((function, owner)) = nominal {
            let application = self.ensure_derived_equality_application(
                ty,
                function,
                hir::DerivedEqualityOrigin::Nominal(owner),
                span,
                &mut Vec::new(),
            )?;
            return Ok(Some(DerivedEqualityCandidate::Nominal {
                overload: CallableCandidate::method(function, owner),
                application,
            }));
        }

        let (function, application) =
            self.ensure_structural_derived_equality_application(ty, span, &mut Vec::new())?;
        Ok(Some(DerivedEqualityCandidate::TypeOwned {
            function,
            application,
        }))
    }

    fn ensure_structural_derived_equality_application(
        &mut self,
        ty: hir::TypeId,
        span: ast::Span,
        stack: &mut Vec<hir::TypeId>,
    ) -> Result<(hir::FunctionId, hir::DerivedEqualityApplicationId), String> {
        if let Some(&application) = self.derived_equality_application_by_type.get(&ty) {
            return Ok((
                self.derived_equality_applications[application].function,
                application,
            ));
        }
        let function = self.declare_structural_derived_equality_method(ty, span);
        let application = self.ensure_derived_equality_application(
            ty,
            function,
            hir::DerivedEqualityOrigin::TypeOwned(ty),
            span,
            stack,
        )?;
        Ok((function, application))
    }

    fn ensure_derived_equality_application(
        &mut self,
        ty: hir::TypeId,
        function: hir::FunctionId,
        origin: hir::DerivedEqualityOrigin,
        span: ast::Span,
        stack: &mut Vec<hir::TypeId>,
    ) -> Result<hir::DerivedEqualityApplicationId, String> {
        if let Some(&application) = self.derived_equality_application_by_type.get(&ty) {
            return Ok(application);
        }
        if stack.contains(&ty) {
            return Err(format!(
                "recursive value layout reaches `{}` without crossing a reference boundary",
                self.type_name(ty)
            ));
        }
        stack.push(ty);
        let body = self.build_derived_equality_body(ty, span, stack);
        let popped = stack.pop();
        debug_assert_eq!(popped, Some(ty));
        let body = body?;
        let application =
            self.derived_equality_applications
                .alloc(hir::DerivedEqualityApplication {
                    function,
                    origin,
                    owner_ty: ty,
                    attributes: self.functions[function].attributes,
                    span,
                    body,
                });
        self.derived_equality_application_by_type
            .insert(ty, application);
        Ok(application)
    }
}
