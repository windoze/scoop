//! Projection of public Export HIR constants into their canonical value table.

use std::fmt;

use scoop_identity::{
    ConeIdentity, DeclarationScope, DefinitionOriginSubject, PersistentPropertyId,
};

use crate::{
    CanonicalConstValueV1, CanonicalExportConstValuesV1, DeclaredVisibility,
    ExportConstValueSetBuildError, ExportConstValueV1, ExportDefinitionSourceV1, ExportHir,
    HirPropertyIdentity, HirSignatureTypeMapper, HirSignatureTypeMappingError, PropertyCapability,
    PropertyId, PropertyRepresentation,
};

impl CanonicalExportConstValuesV1 {
    /// Projects exactly the current Cone's public compile-time properties.
    pub fn from_export_hir(export: &ExportHir) -> Result<Self, ExportConstValueBuildError> {
        let mapper = HirSignatureTypeMapper::new(crate::HirTypeIdentityInputs::from_export(export));
        let mut records = Vec::new();

        for &property_id in &export.public_surface.properties {
            let property = property_declaration(export, property_id).ok_or_else(|| {
                ExportConstValueBuildError::UnknownPublicProperty(raw_property(property_id))
            })?;
            let PropertyRepresentation::Const { value } = &property.representation else {
                continue;
            };
            let identity = export.property_identities.get(property_id).ok_or_else(|| {
                ExportConstValueBuildError::MissingPropertyIdentity(raw_property(property_id))
            })?;
            let HirPropertyIdentity::Ordinary(identity) = identity else {
                return Err(ExportConstValueBuildError::ConstMustBeOrdinary(
                    raw_property(property_id),
                ));
            };
            let persistent = identity.id();
            if identity.key().origin() != export.cone {
                return Err(ExportConstValueBuildError::ForeignDeclaration {
                    property: persistent,
                    expected: export.cone,
                    actual: identity.key().origin(),
                });
            }
            if identity.key().scope() != &DeclarationScope::ConeWide {
                return Err(ExportConstValueBuildError::InvalidDeclarationScope(
                    persistent,
                ));
            }
            if property.access.declared != DeclaredVisibility::Public
                || !property.access.lookup.0.is_universal()
                || property.access.slot.is_some()
            {
                return Err(ExportConstValueBuildError::InvalidPublicAccess(persistent));
            }
            if !matches!(property.capability, PropertyCapability::ReadOnly { .. }) {
                return Err(ExportConstValueBuildError::ConstMustBeReadOnly(persistent));
            }

            let value_type = mapper.map(property.ty, &[]).map_err(|source| {
                ExportConstValueBuildError::Signature {
                    property: persistent,
                    source,
                }
            })?;
            let value = canonical_value(value.clone())
                .ok_or(ExportConstValueBuildError::RawPointerConstant(persistent))?;
            let origin = export
                .export_definition_origins
                .get(DefinitionOriginSubject::Property(persistent))
                .ok_or(ExportConstValueBuildError::MissingDefinitionOrigin(
                    persistent,
                ))?;
            let origin_cone = origin.origin().source().cone();
            if origin_cone != export.cone {
                return Err(ExportConstValueBuildError::ForeignDefinitionOrigin {
                    property: persistent,
                    expected: export.cone,
                    actual: origin_cone,
                });
            }
            records.push(ExportConstValueV1::new(
                persistent,
                value_type,
                value,
                ExportDefinitionSourceV1::new(origin.origin().clone()),
            ));
        }

        Self::try_new(records).map_err(ExportConstValueBuildError::Table)
    }
}

fn property_declaration(export: &ExportHir, property: PropertyId) -> Option<&crate::Property> {
    ((raw_property(property) as usize) < export.properties.len())
        .then(|| &export.properties[property])
}

fn canonical_value(value: crate::HirConstantImage) -> Option<crate::CanonicalValueConstantV1> {
    use crate::{CanonicalValueConstantV1 as C, HirConstantImage as H};
    Some(match value {
        H::Unit => C::Unit,
        H::Tuple(elements) => C::Tuple(
            elements
                .into_iter()
                .map(canonical_value)
                .collect::<Option<_>>()?,
        ),
        H::Struct { fields, .. } => C::Struct(
            fields
                .into_iter()
                .map(canonical_value)
                .collect::<Option<_>>()?,
        ),
        H::Enum { variant, fields } => C::Enum {
            variant: variant.variant,
            fields: fields
                .into_iter()
                .map(canonical_value)
                .collect::<Option<_>>()?,
        },
        H::NullPointer(_) => return None,
        scalar => C::Scalar(
            CanonicalConstValueV1::try_from(scalar).expect("all aggregate images were handled"),
        ),
    })
}

fn raw_property(property: PropertyId) -> u32 {
    property.into_raw().into_u32()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportConstValueBuildError {
    UnknownPublicProperty(u32),
    MissingPropertyIdentity(u32),
    ConstMustBeOrdinary(u32),
    ForeignDeclaration {
        property: PersistentPropertyId,
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    InvalidDeclarationScope(PersistentPropertyId),
    InvalidPublicAccess(PersistentPropertyId),
    ConstMustBeReadOnly(PersistentPropertyId),
    Signature {
        property: PersistentPropertyId,
        source: HirSignatureTypeMappingError,
    },
    RawPointerConstant(PersistentPropertyId),
    MissingDefinitionOrigin(PersistentPropertyId),
    ForeignDefinitionOrigin {
        property: PersistentPropertyId,
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    Table(ExportConstValueSetBuildError),
}

impl fmt::Display for ExportConstValueBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownPublicProperty(property) => {
                write!(
                    formatter,
                    "public property id {property} is outside the HIR arena"
                )
            }
            Self::MissingPropertyIdentity(property) => write!(
                formatter,
                "public const property id {property} has no persistent identity"
            ),
            Self::ConstMustBeOrdinary(property) => write!(
                formatter,
                "public const property id {property} has an extension-property identity"
            ),
            Self::ForeignDeclaration {
                property,
                expected,
                actual,
            } => write!(
                formatter,
                "const property {property} belongs to Cone {actual}, not current Cone {expected}"
            ),
            Self::InvalidDeclarationScope(property) => write!(
                formatter,
                "const property {property} does not have ConeWide declaration scope"
            ),
            Self::InvalidPublicAccess(property) => write!(
                formatter,
                "const property {property} does not have public direct-only lookup access"
            ),
            Self::ConstMustBeReadOnly(property) => {
                write!(formatter, "const property {property} is not read-only")
            }
            Self::Signature { property, source } => {
                write!(
                    formatter,
                    "cannot map const property {property} type: {source}"
                )
            }
            Self::RawPointerConstant(property) => write!(
                formatter,
                "const property {property} contains a raw storage null"
            ),
            Self::MissingDefinitionOrigin(property) => write!(
                formatter,
                "const property {property} has no persistent definition origin"
            ),
            Self::ForeignDefinitionOrigin {
                property,
                expected,
                actual,
            } => write!(
                formatter,
                "const property {property} definition origin belongs to Cone {actual}, not current Cone {expected}"
            ),
            Self::Table(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for ExportConstValueBuildError {}
