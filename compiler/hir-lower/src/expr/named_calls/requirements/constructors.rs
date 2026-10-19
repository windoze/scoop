use super::*;
use crate::expr::named_calls::nominals::PreparedNominalPlans;

impl Lowerer {
    pub(super) fn requirement_constructor_probes(
        &mut self,
        subject: TypeId,
        arguments: &[ast::CallArgument],
        span: Span,
    ) -> Result<Vec<NamedApplicable>, String> {
        let call = ast::CallExpr {
            callee: ast::Ident {
                text: self.type_name(subject),
                span,
            },
            type_args: Vec::new(),
            args: arguments.to_vec(),
            span,
        };
        let nominal = match self.types[subject] {
            Type::Struct(application) => self
                .source_struct_id(self.struct_applications[application].template)
                .map(crate::NominalTarget::Struct),
            Type::Class(application) => self
                .source_class_id(self.class_applications[application].template)
                .map(crate::NominalTarget::Class),
            Type::Ptr(_) => self.ffi_ptr.map(crate::NominalTarget::Struct),
            _ => None,
        };
        if let Some(crate::NominalTarget::Class(class)) = nominal
            && (self.enclosing_class_field(class).is_some()
                || self.captured_environment_field(class).is_some())
        {
            return Err("constructor requirements cannot use an implicit outer instance or runtime captures".into());
        }
        let prepared = if let Some(nominal) = nominal {
            let mut prepared = self
                .nominal_constructor_plans(nominal, &call, Some(subject), true)
                .map_err(|()| self.requirement_failure("type has no public constructor"))?;
            if let PreparedNominalPlans::Nominal(plans, _) = &mut prepared {
                plans.retain(|plan| match plan.view.target {
                    NominalConstructorSource::Struct(id) => {
                        self.struct_constructors[id].access.declared
                            == hir::DeclaredVisibility::Public
                    }
                    NominalConstructorSource::Class(id) => {
                        self.class_constructors[id].access.declared
                            == hir::DeclaredVisibility::Public
                    }
                    _ => true,
                });
            }
            prepared
        } else if let Some(owner) = self.imported_nominal_owner(subject) {
            if self.nominal_enclosing_owner(owner).is_some() {
                return Err(
                    "constructor requirements cannot use an implicit outer instance".into(),
                );
            }
            PreparedNominalPlans::Imported {
                owner,
                expected: Some(subject),
                fixed_alias: true,
                alias: None,
            }
        } else {
            return Err(format!(
                "type `{}` has no public source constructor",
                self.type_name(subject)
            ));
        };
        let mut applicable = Vec::new();
        let mut failures = Vec::new();
        self.collect_prepared_nominal_probes(prepared, &call, &[], &mut applicable, &mut failures);
        if applicable.is_empty()
            && let Some(failure) = failures.first()
        {
            return Err(failure.requirement_failure("constructor requirement is inapplicable"));
        }
        Ok(applicable)
    }
}
