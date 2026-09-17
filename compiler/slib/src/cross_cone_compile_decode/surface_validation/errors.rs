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
    NominalInterfaces(
        Box<NominalInterfaceSetSemanticValidationError<CrossConeHirNominalAuthorityError>>,
    ),
}

#[derive(Debug)]
pub enum CrossConeHirPropertySurfaceError {
    PropertyInterfaces(
        Box<PropertyInterfaceSetSemanticValidationError<CrossConeHirNominalAuthorityError>>,
    ),
}

#[derive(Debug)]
pub enum CrossConeHirCallableSurfaceError {
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

impl_surface_error!(CrossConeHirNominalSurfaceError, NominalInterfaces);
impl_surface_error!(CrossConeHirPropertySurfaceError, PropertyInterfaces);
impl_surface_error!(CrossConeHirCallableSurfaceError, CallableInterfaces);
impl_surface_error!(CrossConeHirTypeAliasSurfaceError, TypeAliasInterfaces);
