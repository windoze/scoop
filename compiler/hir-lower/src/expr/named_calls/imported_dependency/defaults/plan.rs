use std::collections::BTreeMap;
use std::fmt;

use scoop_hir as hir;
use scoop_identity::LocalValueSelector;

use super::super::arguments::{ImportedArgumentMap, ImportedParameterInput};
use crate::Lowerer;
use crate::imported_capabilities::ImportedCapabilityRequirement;

#[derive(Clone)]
pub(in super::super) struct ImportedDefaultPlan {
    templates: BTreeMap<hir::ExportDefaultTemplateKeyV1, PreparedImportedDefault>,
}

#[derive(Clone)]
pub(crate) struct PreparedImportedDefault {
    pub(super) bindings: crate::imported_core::ImportedTypeBindings,
    pub(super) template: hir::ExportDefaultTemplateV1,
    pub(super) callables:
        BTreeMap<scoop_identity::CallableTemplateOrigin, hir::ImportedCallableDeclaration>,
}

impl ImportedDefaultPlan {
    pub(super) fn empty() -> Self {
        Self {
            templates: BTreeMap::new(),
        }
    }

    pub(in super::super) fn get(
        &self,
        key: hir::ExportDefaultTemplateKeyV1,
    ) -> Option<&PreparedImportedDefault> {
        self.templates.get(&key)
    }
}

impl Lowerer {
    pub(in super::super) fn prepare_imported_defaults(
        &mut self,
        candidate: &dyn hir::ImportedCallableSource,
        arguments: &ImportedArgumentMap,
    ) -> Result<ImportedDefaultPlan, ImportedDefaultPlanError> {
        self.prepare_imported_defaults_with_bindings(
            candidate,
            arguments,
            &crate::imported_core::ImportedTypeBindings::new(),
        )
    }

    pub(in super::super) fn prepare_imported_defaults_with_bindings(
        &mut self,
        candidate: &dyn hir::ImportedCallableSource,
        arguments: &ImportedArgumentMap,
        bindings: &crate::imported_core::ImportedTypeBindings,
    ) -> Result<ImportedDefaultPlan, ImportedDefaultPlanError> {
        let mut plan = ImportedDefaultPlan::empty();
        for input in arguments.parameters() {
            let ImportedParameterInput::Default(key) = input else {
                continue;
            };
            if plan.templates.contains_key(key) {
                continue;
            }
            let template = candidate
                .default_template(*key)
                .ok_or(ImportedDefaultPlanError::MissingTemplate(*key))?
                .clone();
            let prepared =
                self.prepare_imported_default_with_bindings(candidate, template, bindings)?;
            plan.templates.insert(*key, prepared);
        }
        Ok(plan)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ImportedDefaultPlanError {
    MissingTemplate(hir::ExportDefaultTemplateKeyV1),
    UnknownLocal(LocalValueSelector),
    UnboundCapture(u32),
    InvalidControlFlow(&'static str),
    Callable {
        callee: scoop_identity::CallableTemplateOrigin,
        error: String,
    },
    Requires {
        requirement: ImportedCapabilityRequirement,
        operation: &'static str,
    },
}

impl fmt::Display for ImportedDefaultPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnboundCapture(index) => write!(
                formatter,
                "dependency default root has no closure input {index}"
            ),
            Self::MissingTemplate(key) => {
                write!(
                    formatter,
                    "dependency callable is missing default template {key:?}"
                )
            }
            Self::UnknownLocal(local) => {
                write!(
                    formatter,
                    "dependency default reads unmapped local {local:?}"
                )
            }
            Self::InvalidControlFlow(operation) => {
                write!(
                    formatter,
                    "invalid dependency default control flow: {operation}"
                )
            }
            Self::Callable { callee, error } => {
                write!(
                    formatter,
                    "cannot bind dependency default call {callee:?}: {error}"
                )
            }
            Self::Requires {
                requirement,
                operation,
            } => formatter.write_str(&requirement.diagnostic(operation)),
        }
    }
}

impl std::error::Error for ImportedDefaultPlanError {}

/// Calls use their actual source declaration as the plan key; applications
/// retain their symbolic arguments on each template expression.
pub(super) fn default_callable_origin(
    callee: &hir::DefaultCallableRefV1,
) -> Result<scoop_identity::CallableTemplateOrigin, ImportedDefaultPlanError> {
    use scoop_identity::CallableTemplateOrigin as Origin;
    match callee.declaration() {
        hir::DefaultCallableDeclarationV1::Function(id) => Ok(Origin::Function(id)),
        hir::DefaultCallableDeclarationV1::PropertyAccessor(id) => Ok(Origin::Accessor(id)),
        hir::DefaultCallableDeclarationV1::GenericFunction(id) => Ok(Origin::GenericFunction(id)),
        hir::DefaultCallableDeclarationV1::Generated(_) => {
            Err(ImportedDefaultPlanError::Requires {
                requirement: ImportedCapabilityRequirement::Generic,
                operation: "dependency default generic or lexical call",
            })
        }
    }
}
