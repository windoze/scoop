//! Errors produced by per-artifact HIR surface validation states.

use scoop_hir::{
    CallableInterfaceSetSemanticValidationError, CrossConeHirInternalClosureValidationError,
    NominalInterfaceSetSemanticValidationError, PropertyInterfaceSetSemanticValidationError,
    TypeAliasInterfaceSetSemanticValidationError,
};

use crate::cross_cone_hir_authority::CrossConeHirNominalAuthorityError;

#[derive(Debug)]
pub enum CrossConeHirInternalClosureError {
    Interface(CrossConeHirInternalClosureValidationError),
}

impl std::fmt::Display for CrossConeHirInternalClosureError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Interface(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CrossConeHirInternalClosureError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Interface(error) => Some(error),
        }
    }
}

#[derive(Debug)]
pub enum CrossConeHirNominalSurfaceError {
    Declarations(CrossConeHirNominalAuthorityError),
    Fields(scoop_hir::NominalSourceFieldInventoryError),
    Relations(scoop_hir::NominalDeclarationInventoryError),
    NominalInterfaces(
        Box<NominalInterfaceSetSemanticValidationError<CrossConeHirNominalAuthorityError>>,
    ),
}

#[derive(Debug)]
pub enum CrossConeHirPropertySurfaceError {
    Declarations(CrossConeHirNominalAuthorityError),
    PropertyInterfaces(
        Box<PropertyInterfaceSetSemanticValidationError<CrossConeHirNominalAuthorityError>>,
    ),
}

#[derive(Debug)]
pub enum CrossConeHirCallableSurfaceError {
    Declarations(CrossConeHirNominalAuthorityError),
    Intrinsics(Box<crate::CrossConeIntrinsicDeclarationError>),
    CallableInterfaces(
        Box<CallableInterfaceSetSemanticValidationError<CrossConeHirNominalAuthorityError>>,
    ),
}

#[derive(Debug)]
pub enum CrossConeHirTypeAliasSurfaceError {
    TypeAliasInterfaces(
        Box<TypeAliasInterfaceSetSemanticValidationError<CrossConeHirNominalAuthorityError>>,
    ),
}

macro_rules! impl_surface_error {
    ($error:ident, $variant:ident) => {
        impl std::fmt::Display for $error {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    Self::$variant(error) => error.fmt(formatter),
                }
            }
        }

        impl std::error::Error for $error {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                match self {
                    Self::$variant(error) => Some(error),
                }
            }
        }
    };
}

impl std::fmt::Display for CrossConeHirNominalSurfaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Declarations(error) => error.fmt(f),
            Self::NominalInterfaces(error) => error.fmt(f),
            Self::Fields(error) => error.fmt(f),
            Self::Relations(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for CrossConeHirNominalSurfaceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Declarations(error) => Some(error),
            Self::NominalInterfaces(error) => Some(error.as_ref()),
            Self::Fields(error) => Some(error),
            Self::Relations(error) => Some(error),
        }
    }
}
impl std::fmt::Display for CrossConeHirPropertySurfaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Declarations(error) => error.fmt(f),
            Self::PropertyInterfaces(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for CrossConeHirPropertySurfaceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Declarations(error) => Some(error),
            Self::PropertyInterfaces(error) => Some(error.as_ref()),
        }
    }
}
impl std::fmt::Display for CrossConeHirCallableSurfaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Declarations(error) => error.fmt(f),
            Self::CallableInterfaces(error) => error.fmt(f),
            Self::Intrinsics(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for CrossConeHirCallableSurfaceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Declarations(error) => Some(error),
            Self::CallableInterfaces(error) => Some(error.as_ref()),
            Self::Intrinsics(error) => Some(error.as_ref()),
        }
    }
}
impl_surface_error!(CrossConeHirTypeAliasSurfaceError, TypeAliasInterfaces);
