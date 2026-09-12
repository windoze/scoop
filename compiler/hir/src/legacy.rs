use std::ops::Deref;

use scoop_ast::Diagnostic;

use crate::{
    ExportHir, FunctionGenericity, FunctionId, FunctionKind, HirNativeBoundaryTypeDefinitions,
    LocalConcreteHir, concrete,
};

/// Why an HIR graph cannot be wrapped as a legacy executable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyExecutableEntryError {
    MissingFunction,
    NotTopLevel,
    InvalidEntryShape,
}

impl std::fmt::Display for LegacyExecutableEntryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::MissingFunction => "the legacy entry does not belong to the module",
            Self::NotTopLevel => "the legacy entry is not a top-level function",
            Self::InvalidEntryShape => "the legacy entry is not an ordinary `fun main(): Unit`",
        })
    }
}

impl std::error::Error for LegacyExecutableEntryError {}

/// Temporary M22-compatible executable view over export HIR.
///
/// The base graph is intentionally private and immutable through this wrapper,
/// so its required typed entry cannot be invalidated after construction.
///
/// ```compile_fail
/// fn require_mutable_module<
///     T: std::ops::DerefMut<Target = scoop_hir::ExportHir>,
/// >() {}
/// require_mutable_module::<scoop_hir::LegacyExecutableExportHir>();
/// ```
#[derive(Debug, Clone)]
pub struct LegacyExecutableExportHir {
    module: ExportHir,
    entry: FunctionId,
}

impl LegacyExecutableExportHir {
    pub fn try_new(
        module: ExportHir,
        entry: FunctionId,
    ) -> Result<Self, LegacyExecutableEntryError> {
        validate_export_entry(&module.functions, &module.top_level, module.unit, entry)?;
        Ok(Self { module, entry })
    }

    pub const fn module(&self) -> &ExportHir {
        &self.module
    }

    pub const fn entry(&self) -> FunctionId {
        self.entry
    }

    pub fn into_module(self) -> ExportHir {
        self.module
    }
}

impl Deref for LegacyExecutableExportHir {
    type Target = ExportHir;

    fn deref(&self) -> &Self::Target {
        self.module()
    }
}

/// Temporary M22-compatible executable view over local concrete HIR.
#[derive(Debug, Clone)]
pub struct LegacyExecutableLocalHir {
    module: LocalConcreteHir,
    entry: concrete::FunctionId,
}

impl LegacyExecutableLocalHir {
    pub fn try_new(
        module: LocalConcreteHir,
        entry: concrete::FunctionId,
    ) -> Result<Self, LegacyExecutableEntryError> {
        validate_local_entry(&module.functions, &module.top_level, module.unit, entry)?;
        Ok(Self { module, entry })
    }

    pub const fn module(&self) -> &LocalConcreteHir {
        &self.module
    }

    pub const fn entry(&self) -> concrete::FunctionId {
        self.entry
    }

    pub fn into_module(self) -> LocalConcreteHir {
        self.module
    }
}

impl Deref for LegacyExecutableLocalHir {
    type Target = LocalConcreteHir;

    fn deref(&self) -> &Self::Target {
        self.module()
    }
}

/// Complete legacy executable HIR products for the temporary M22 driver path.
#[derive(Debug, Clone)]
pub struct LegacyExecutableOutput {
    pub export: LegacyExecutableExportHir,
    pub local: LegacyExecutableLocalHir,
    pub native_boundary_types: HirNativeBoundaryTypeDefinitions,
    pub warnings: Vec<Diagnostic>,
}

fn validate_export_entry(
    functions: &la_arena::Arena<crate::Function>,
    top_level: &[FunctionId],
    unit: crate::TypeId,
    entry: FunctionId,
) -> Result<(), LegacyExecutableEntryError> {
    let function = functions
        .iter()
        .find_map(|(id, function)| (id == entry).then_some(function))
        .ok_or(LegacyExecutableEntryError::MissingFunction)?;
    if !top_level.contains(&entry) {
        return Err(LegacyExecutableEntryError::NotTopLevel);
    }
    if function.name != "main"
        || function.method.is_some()
        || !matches!(&function.genericity, FunctionGenericity::Plain)
        || function.is_suspend
        || !function.params.is_empty()
        || function.return_ty != unit
        || !matches!(&function.kind, FunctionKind::User(_))
    {
        return Err(LegacyExecutableEntryError::InvalidEntryShape);
    }
    Ok(())
}

fn validate_local_entry(
    functions: &la_arena::Arena<concrete::Function>,
    top_level: &[concrete::FunctionId],
    unit: concrete::TypeId,
    entry: concrete::FunctionId,
) -> Result<(), LegacyExecutableEntryError> {
    let function = functions
        .iter()
        .find_map(|(id, function)| (id == entry).then_some(function))
        .ok_or(LegacyExecutableEntryError::MissingFunction)?;
    if !top_level.contains(&entry) {
        return Err(LegacyExecutableEntryError::NotTopLevel);
    }
    if function.name != "main"
        || function.method.is_some()
        || !matches!(
            function.materialization.template(),
            concrete::CallableTemplateOwner::Function(_)
        )
        || function.materialization.context()
            != concrete::CallableMaterializationContext::NoSubstitution
        || function.is_suspend
        || !function.params.is_empty()
        || function.return_ty != unit
        || !matches!(&function.kind, concrete::FunctionKind::User(_))
    {
        return Err(LegacyExecutableEntryError::InvalidEntryShape);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use la_arena::Arena;
    use scoop_ast::Span;
    use scoop_identity::{
        CallableMaterialization, CallableMaterializationContext, CallableTemplateOwner,
        CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope,
        DefinitionOwnerChain, PackagePath, PersistentFunctionId, SourceDeclarationKey,
        SourceDeclarationSite,
    };

    use super::*;

    fn export_user_body() -> FunctionKind {
        FunctionKind::User(crate::Body {
            locals: Arena::new(),
            statements: Vec::new(),
        })
    }

    fn export_function(unit: crate::TypeId, kind: FunctionKind) -> crate::Function {
        crate::Function {
            name: "main".to_string(),
            access: crate::DeclarationAccess::public(),
            override_access: Vec::new(),
            genericity: FunctionGenericity::Plain,
            is_suspend: false,
            modifiers: crate::CallableModifiers::default(),
            params: Vec::new(),
            return_ty: unit,
            attributes: crate::FunctionAttributes::default(),
            kind,
            method: None,
            span: Span::new(0, 0),
        }
    }

    fn local_function(unit: concrete::TypeId) -> concrete::Function {
        concrete::Function {
            name: "main".to_string(),
            materialization: CallableMaterialization::new(
                CallableTemplateOwner::Function(persistent_function("main")),
                CallableMaterializationContext::NoSubstitution,
            ),
            is_suspend: false,
            modifiers: crate::CallableModifiers::default(),
            params: Vec::new(),
            return_ty: unit,
            attributes: crate::FunctionAttributes::default(),
            kind: concrete::FunctionKind::User(concrete::Body {
                locals: Arena::new(),
                statements: Vec::new(),
            }),
            method: None,
            span: Span::new(0, 0),
        }
    }

    fn persistent_function(name: &str) -> PersistentFunctionId {
        let site = SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap();
        CborIdentityRecord::from_key(SourceDeclarationKey::function(
            site,
            CanonicalIdentifier::new(name).unwrap(),
            0,
            None,
            Vec::new(),
        ))
        .unwrap()
        .id()
    }

    #[test]
    fn export_entry_validation_accepts_valid_and_rejects_missing_non_top_level_and_foreign() {
        let unit = crate::TypeId::from_raw(0_u32.into());
        let mut functions = Arena::new();
        let valid = functions.alloc(export_function(unit, export_user_body()));
        assert_eq!(
            validate_export_entry(&functions, &[valid], unit, valid),
            Ok(())
        );
        assert_eq!(
            validate_export_entry(&functions, &[], unit, valid),
            Err(LegacyExecutableEntryError::NotTopLevel)
        );
        assert_eq!(
            validate_export_entry(
                &functions,
                &[valid],
                unit,
                FunctionId::from_raw(u32::MAX.into()),
            ),
            Err(LegacyExecutableEntryError::MissingFunction)
        );

        let foreign = functions.alloc(export_function(
            unit,
            FunctionKind::Extern(crate::ExternFunctionId::from_raw(0_u32.into())),
        ));
        assert_eq!(
            validate_export_entry(&functions, &[foreign], unit, foreign),
            Err(LegacyExecutableEntryError::InvalidEntryShape)
        );
    }

    #[test]
    fn export_entry_validation_rejects_an_invalid_signature_shape() {
        let unit = crate::TypeId::from_raw(0_u32.into());
        let not_unit = crate::TypeId::from_raw(1_u32.into());
        let valid = || export_function(unit, export_user_body());
        let invalid = [
            ("name", {
                let mut function = valid();
                function.name = "notMain".to_string();
                function
            }),
            ("member", {
                let mut function = valid();
                function.method = Some(crate::Method {
                    owner: unit,
                    modifier: crate::MethodModifier::Final,
                    dispatch: crate::MethodDispatch::Direct,
                });
                function
            }),
            ("genericity", {
                let mut function = valid();
                function.genericity = FunctionGenericity::Generic {
                    definition: crate::GenericFunctionId::from_raw(0_u32.into()),
                    parameters: Vec::new(),
                };
                function
            }),
            ("suspend", {
                let mut function = valid();
                function.is_suspend = true;
                function
            }),
            ("parameters", {
                let mut function = valid();
                function.params.push(crate::Param {
                    name: "value".to_string(),
                    ty: unit,
                    local: crate::LocalId::from_raw(0_u32.into()),
                });
                function
            }),
            ("return type", {
                let mut function = valid();
                function.return_ty = not_unit;
                function
            }),
            ("kind", export_function(unit, FunctionKind::DerivedEquality)),
        ];
        for (axis, function) in invalid {
            let mut functions = Arena::new();
            let invalid = functions.alloc(function);
            assert_eq!(
                validate_export_entry(&functions, &[invalid], unit, invalid),
                Err(LegacyExecutableEntryError::InvalidEntryShape),
                "export legacy entry accepted an invalid {axis}"
            );
        }
    }

    #[test]
    fn local_entry_validation_accepts_valid_and_rejects_missing_and_non_top_level() {
        let unit = concrete::TypeId::from_raw(0_u32.into());
        let mut functions = Arena::new();
        let valid = functions.alloc(local_function(unit));
        assert_eq!(
            validate_local_entry(&functions, &[valid], unit, valid),
            Ok(())
        );
        assert_eq!(
            validate_local_entry(&functions, &[], unit, valid),
            Err(LegacyExecutableEntryError::NotTopLevel)
        );
        assert_eq!(
            validate_local_entry(
                &functions,
                &[valid],
                unit,
                concrete::FunctionId::from_raw(u32::MAX.into()),
            ),
            Err(LegacyExecutableEntryError::MissingFunction)
        );
    }

    #[test]
    fn local_entry_validation_rejects_every_invalid_shape_axis() {
        let unit = concrete::TypeId::from_raw(0_u32.into());
        let not_unit = concrete::TypeId::from_raw(1_u32.into());
        let valid = || local_function(unit);
        let invalid = [
            ("name", {
                let mut function = valid();
                function.name = "notMain".to_string();
                function
            }),
            ("member", {
                let mut function = valid();
                function.method = Some(concrete::Method {
                    owner: unit,
                    modifier: crate::MethodModifier::Final,
                    dispatch: concrete::MethodDispatch::Direct,
                });
                function
            }),
            ("suspend", {
                let mut function = valid();
                function.is_suspend = true;
                function
            }),
            ("parameters", {
                let mut function = valid();
                function.params.push(concrete::Param {
                    name: "value".to_string(),
                    ty: unit,
                    local: concrete::LocalId::from_raw(0_u32.into()),
                });
                function
            }),
            ("return type", {
                let mut function = valid();
                function.return_ty = not_unit;
                function
            }),
            ("kind", {
                let mut function = valid();
                function.kind = concrete::FunctionKind::Extern(
                    concrete::ExternFunctionId::from_raw(0_u32.into()),
                );
                function
            }),
        ];
        for (axis, function) in invalid {
            let mut functions = Arena::new();
            let invalid = functions.alloc(function);
            assert_eq!(
                validate_local_entry(&functions, &[invalid], unit, invalid),
                Err(LegacyExecutableEntryError::InvalidEntryShape),
                "local legacy entry accepted an invalid {axis}"
            );
        }
    }
}
