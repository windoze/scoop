use std::collections::{BTreeMap, BTreeSet};

use scoop_hir as hir;
use scoop_identity::LocalValueSelector;

use super::super::plan::{ImportedDefaultPlanError, PreparedImportedDefaultCallable};
use crate::Lowerer;
use crate::imported_capabilities::ImportedCapabilityRequirement;

impl Lowerer {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn preflight_imported_default_statements(
        &mut self,
        owner: &hir::ImportedDependencyCallableCandidate,
        template: &hir::ExportDefaultTemplateV1,
        statements: &[hir::DefaultStatementV1],
        locals: &BTreeSet<LocalValueSelector>,
        callables: &mut BTreeMap<hir::DefaultCallableRefV1, PreparedImportedDefaultCallable>,
        loop_depth: usize,
    ) -> Result<(), ImportedDefaultPlanError> {
        for statement in statements {
            self.preflight_imported_default_statement(
                owner, template, statement, locals, callables, loop_depth,
            )?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn preflight_imported_default_statement(
        &mut self,
        owner: &hir::ImportedDependencyCallableCandidate,
        template: &hir::ExportDefaultTemplateV1,
        statement: &hir::DefaultStatementV1,
        locals: &BTreeSet<LocalValueSelector>,
        callables: &mut BTreeMap<hir::DefaultCallableRefV1, PreparedImportedDefaultCallable>,
        loop_depth: usize,
    ) -> Result<(), ImportedDefaultPlanError> {
        use hir::DefaultStatementKindV1 as Kind;
        match statement.kind() {
            Kind::Expr(value) => self
                .preflight_imported_default_expression(owner, template, value, locals, callables),
            Kind::ValDecl { pattern, init } => {
                self.preflight_imported_default_pattern(pattern, locals)?;
                self.preflight_imported_default_expression(owner, template, init, locals, callables)
            }
            Kind::Assign { target, value } => {
                match target.as_ref() {
                    hir::DefaultAssignTargetV1::Local { local } if locals.contains(local) => {}
                    hir::DefaultAssignTargetV1::Local { local } => {
                        return Err(ImportedDefaultPlanError::UnknownLocal(local.clone()));
                    }
                    hir::DefaultAssignTargetV1::Global { .. }
                    | hir::DefaultAssignTargetV1::Index { .. }
                    | hir::DefaultAssignTargetV1::Field { .. } => {
                        return Err(ImportedDefaultPlanError::Requires {
                            requirement: ImportedCapabilityRequirement::Layout,
                            operation: "dependency default assignment target",
                        });
                    }
                }
                self.preflight_imported_default_expression(
                    owner, template, value, locals, callables,
                )
            }
            Kind::If {
                condition,
                then_body,
                else_body,
            } => {
                self.preflight_imported_default_expression(
                    owner, template, condition, locals, callables,
                )?;
                self.preflight_imported_default_statements(
                    owner, template, then_body, locals, callables, loop_depth,
                )?;
                if let hir::OptionalDefaultStatementListViewV1::Present(body) = else_body.view() {
                    self.preflight_imported_default_statements(
                        owner, template, body, locals, callables, loop_depth,
                    )?;
                }
                Ok(())
            }
            Kind::While {
                condition_setup,
                condition,
                body,
            } => {
                let nested_depth = loop_depth.checked_add(1).ok_or(
                    ImportedDefaultPlanError::InvalidControlFlow("loop nesting overflow"),
                )?;
                self.preflight_imported_default_statements(
                    owner,
                    template,
                    condition_setup,
                    locals,
                    callables,
                    nested_depth,
                )?;
                self.preflight_imported_default_expression(
                    owner, template, condition, locals, callables,
                )?;
                self.preflight_imported_default_statements(
                    owner,
                    template,
                    body,
                    locals,
                    callables,
                    nested_depth,
                )
            }
            Kind::Break | Kind::Continue if loop_depth != 0 => Ok(()),
            Kind::Break => Err(ImportedDefaultPlanError::InvalidControlFlow(
                "break without an enclosing loop",
            )),
            Kind::Continue => Err(ImportedDefaultPlanError::InvalidControlFlow(
                "continue without an enclosing loop",
            )),
            Kind::When(value) => self.preflight_imported_default_when(
                owner, template, value, locals, callables, loop_depth,
            ),
            Kind::Return(value) => {
                if let Some(value) = value.as_ref() {
                    self.preflight_imported_default_expression(
                        owner, template, value, locals, callables,
                    )?;
                }
                Ok(())
            }
            Kind::InitializationEnsure(_) => Err(ImportedDefaultPlanError::Requires {
                requirement: ImportedCapabilityRequirement::Layout,
                operation: "dependency default initialization unit",
            }),
            Kind::LocalFunction(_) => Err(ImportedDefaultPlanError::Requires {
                requirement: ImportedCapabilityRequirement::Generic,
                operation: "dependency default local function",
            }),
            Kind::For(_) => Err(ImportedDefaultPlanError::Requires {
                requirement: ImportedCapabilityRequirement::Generic,
                operation: "dependency default iteration protocol",
            }),
            Kind::Try(_) | Kind::Throw(_) => Err(ImportedDefaultPlanError::Requires {
                requirement: ImportedCapabilityRequirement::Layout,
                operation: "dependency default exception value",
            }),
        }
    }

    fn preflight_imported_default_pattern(
        &self,
        pattern: &hir::DefaultPatternV1,
        locals: &BTreeSet<LocalValueSelector>,
    ) -> Result<(), ImportedDefaultPlanError> {
        match pattern.view() {
            hir::DefaultPatternViewV1::Binding { local } if locals.contains(local) => Ok(()),
            hir::DefaultPatternViewV1::Binding { local } => {
                Err(ImportedDefaultPlanError::UnknownLocal(local.clone()))
            }
            hir::DefaultPatternViewV1::Wildcard => Ok(()),
            hir::DefaultPatternViewV1::Literal { .. }
            | hir::DefaultPatternViewV1::Variant { .. }
            | hir::DefaultPatternViewV1::Tuple { .. }
            | hir::DefaultPatternViewV1::Struct { .. } => Err(ImportedDefaultPlanError::Requires {
                requirement: ImportedCapabilityRequirement::Layout,
                operation: "dependency default destructuring pattern",
            }),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn preflight_imported_default_when(
        &mut self,
        owner: &hir::ImportedDependencyCallableCandidate,
        template: &hir::ExportDefaultTemplateV1,
        value: &hir::DefaultWhenV1,
        locals: &BTreeSet<LocalValueSelector>,
        callables: &mut BTreeMap<hir::DefaultCallableRefV1, PreparedImportedDefaultCallable>,
        loop_depth: usize,
    ) -> Result<(), ImportedDefaultPlanError> {
        self.preflight_imported_default_expression(
            owner,
            template,
            value.subject(),
            locals,
            callables,
        )?;
        for arm in value.arms() {
            self.preflight_imported_default_pattern(arm.pattern(), locals)?;
            if let Some(guard) = arm.guard().as_ref() {
                self.preflight_imported_default_statements(
                    owner,
                    template,
                    guard.setup(),
                    locals,
                    callables,
                    loop_depth,
                )?;
                self.preflight_imported_default_expression(
                    owner,
                    template,
                    guard.condition(),
                    locals,
                    callables,
                )?;
            }
            self.preflight_imported_default_statements(
                owner,
                template,
                arm.body(),
                locals,
                callables,
                loop_depth,
            )?;
        }
        match value.fallback().view() {
            hir::DefaultWhenFallbackViewV1::Else(body) => self
                .preflight_imported_default_statements(
                    owner, template, body, locals, callables, loop_depth,
                ),
            hir::DefaultWhenFallbackViewV1::IrrefutableArm { subject_type }
            | hir::DefaultWhenFallbackViewV1::PatternMatrix { subject_type } => {
                self.imported_default_core_type(subject_type).map(|_| ())
            }
            hir::DefaultWhenFallbackViewV1::EnumPatternMatrix { .. } => {
                Err(ImportedDefaultPlanError::Requires {
                    requirement: ImportedCapabilityRequirement::Layout,
                    operation: "dependency default enum exhaustiveness proof",
                })
            }
        }
    }
}
