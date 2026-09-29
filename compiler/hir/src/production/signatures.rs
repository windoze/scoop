//! Shared projection of HIR types and type-parameter binders into public
//! cross-Cone signatures.

use std::fmt;

use scoop_identity::{CanonicalIdentifier, CanonicalIdentifierError, SignatureTypeKey};

use crate::{
    CanonicalBinderListV1, CanonicalSignatureTypesV1, ExportHir, HirSignatureBinder,
    HirSignatureTypeMapper, HirSignatureTypeMappingError, NominalTypeParameterBoundsV1,
    SignatureTypeSetBuildError, TypeParamBounds, TypeParamDecl, TypeParameterBinderBuildError,
    TypeParameterBinderV1, TypeParameterBoundsBuildError, TypeParameterBoundsV1,
};

pub(super) struct HirInterfaceSignatureProjector<'a> {
    mapper: HirSignatureTypeMapper<'a>,
}

impl<'a> HirInterfaceSignatureProjector<'a> {
    pub(super) fn new(export: &'a ExportHir) -> Self {
        Self {
            mapper: HirSignatureTypeMapper::new(crate::HirTypeIdentityInputs::from_export(export)),
        }
    }

    pub(super) fn map_type(
        &self,
        ty: crate::TypeId,
        binders: &[HirSignatureBinder],
    ) -> Result<SignatureTypeKey, HirInterfaceSignatureProjectionError> {
        self.mapper
            .map(ty, binders)
            .map_err(HirInterfaceSignatureProjectionError::Type)
    }

    pub(super) fn binder_frame(
        &self,
        parameters: &[TypeParamDecl],
        depth: u32,
    ) -> Result<Vec<HirSignatureBinder>, HirInterfaceSignatureProjectionError> {
        parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| {
                let index = u32::try_from(index)
                    .map_err(|_| HirInterfaceSignatureProjectionError::TooManyTypeParameters)?;
                Ok(HirSignatureBinder {
                    parameter: parameter.id,
                    depth,
                    index,
                })
            })
            .collect()
    }

    pub(super) fn function_binders(
        &self,
        function: &crate::Function,
    ) -> Result<Vec<HirSignatureBinder>, HirInterfaceSignatureProjectionError> {
        match &function.genericity {
            crate::FunctionGenericity::Plain => Ok(Vec::new()),
            crate::FunctionGenericity::Generic { parameters, .. } => {
                self.binder_frame(parameters, 0)
            }
            crate::FunctionGenericity::OwnerParameterizedMethod {
                owner_parameters, ..
            } => self.binder_frame(owner_parameters, 0),
            crate::FunctionGenericity::GenericMethod {
                owner_parameters,
                method_parameters,
                ..
            } => method_parameters
                .iter()
                .enumerate()
                .map(|(index, parameter)| (index, parameter, 0))
                .chain(
                    owner_parameters
                        .iter()
                        .enumerate()
                        .map(|(index, parameter)| (index, parameter, 1)),
                )
                .map(|(index, parameter, depth)| {
                    Ok(HirSignatureBinder {
                        parameter: parameter.id,
                        depth,
                        index: u32::try_from(index).map_err(|_| {
                            HirInterfaceSignatureProjectionError::TooManyTypeParameters
                        })?,
                    })
                })
                .collect(),
        }
    }

    pub(super) fn project_binder_list(
        &self,
        parameters: &[TypeParamDecl],
        visible_binders: &[HirSignatureBinder],
    ) -> Result<CanonicalBinderListV1, HirInterfaceSignatureProjectionError> {
        self.project_binder_iter(parameters.iter(), visible_binders)
    }

    pub(super) fn project_binder_iter<'p>(
        &self,
        parameters: impl Iterator<Item = &'p TypeParamDecl>,
        visible_binders: &[HirSignatureBinder],
    ) -> Result<CanonicalBinderListV1, HirInterfaceSignatureProjectionError> {
        let mut projected = Vec::with_capacity(parameters.size_hint().0);
        for (index, parameter) in parameters.enumerate() {
            let index = u32::try_from(index)
                .map_err(|_| HirInterfaceSignatureProjectionError::TooManyTypeParameters)?;
            let name = CanonicalIdentifier::new(&parameter.name).map_err(|source| {
                HirInterfaceSignatureProjectionError::ParameterName { index, source }
            })?;
            let bounds = self.project_bounds(&parameter.bounds, visible_binders)?;
            projected.push(TypeParameterBinderV1::new(name, bounds));
        }
        CanonicalBinderListV1::try_new(projected)
            .map_err(HirInterfaceSignatureProjectionError::BinderList)
    }

    fn project_bounds(
        &self,
        bounds: &TypeParamBounds,
        binders: &[HirSignatureBinder],
    ) -> Result<TypeParameterBoundsV1, HirInterfaceSignatureProjectionError> {
        match bounds {
            TypeParamBounds::Unconstrained => Ok(TypeParameterBoundsV1::Unconstrained),
            TypeParamBounds::Value { .. } => Ok(TypeParameterBoundsV1::Value),
            TypeParamBounds::Ref { .. } => Ok(TypeParameterBoundsV1::Ref),
            TypeParamBounds::Nominal(bounds) => {
                let class = bounds
                    .class
                    .as_ref()
                    .map(|bound| self.map_type(bound.ty, binders))
                    .transpose()?;
                let interfaces = bounds
                    .interfaces
                    .iter()
                    .map(|bound| self.map_type(bound.ty, binders))
                    .collect::<Result<Vec<_>, _>>()?;
                let interfaces = CanonicalSignatureTypesV1::try_new(interfaces)
                    .map_err(HirInterfaceSignatureProjectionError::InterfaceBounds)?;
                NominalTypeParameterBoundsV1::try_new(class, interfaces)
                    .map(TypeParameterBoundsV1::Nominal)
                    .map_err(HirInterfaceSignatureProjectionError::NominalBounds)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirInterfaceSignatureProjectionError {
    TooManyTypeParameters,
    ParameterName {
        index: u32,
        source: CanonicalIdentifierError,
    },
    Type(HirSignatureTypeMappingError),
    InterfaceBounds(SignatureTypeSetBuildError),
    NominalBounds(TypeParameterBoundsBuildError),
    BinderList(TypeParameterBinderBuildError),
}

impl fmt::Display for HirInterfaceSignatureProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyTypeParameters => {
                formatter.write_str("HIR type-parameter count exceeds u32")
            }
            Self::ParameterName { index, source } => {
                write!(
                    formatter,
                    "invalid HIR type-parameter name at index {index}: {source}"
                )
            }
            Self::Type(source) => source.fmt(formatter),
            Self::InterfaceBounds(source) => source.fmt(formatter),
            Self::NominalBounds(source) => source.fmt(formatter),
            Self::BinderList(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for HirInterfaceSignatureProjectionError {}
