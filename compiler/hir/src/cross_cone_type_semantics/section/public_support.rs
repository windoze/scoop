use super::*;
use scoop_identity::ConeIdentity;
use scoop_wire::{BudgetMeter, WirePath};

/// Borrowed evidence from the complete frozen ten-table validator. A single
/// callable/nominal table or an unvalidated section cannot construct this token.
#[derive(Clone, Copy, Debug)]
pub struct CheckedTypeSectionPublicSupportV1<'a> {
    provider: ConeIdentity,
    section: &'a CrossConeHirInterfaceSectionV1,
}
impl<'a> CheckedTypeSectionPublicSupportV1<'a> {
    pub fn validate<A: CrossConeHirInterfaceSemanticAuthority<E>, E>(
        section: &'a CrossConeHirInterfaceSectionV1,
        provider: ConeIdentity,
        surface: &CanonicalDirectPublicSurfaceV1,
        authority: &mut A,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Self, TypeSectionPublicSupportError<E>> {
        if ExportDefinitionSourceSemanticAuthority::current_cone(authority) != provider {
            return Err(TypeSectionPublicSupportError::Provider);
        }
        section
            .validate_semantics(provider, surface, authority, meter, path)
            .map_err(|error| TypeSectionPublicSupportError::Section(Box::new(error)))?;
        Ok(Self { provider, section })
    }
    pub const fn provider(self) -> ConeIdentity {
        self.provider
    }
    pub const fn section(self) -> &'a CrossConeHirInterfaceSectionV1 {
        self.section
    }
}

#[derive(Debug)]
pub enum TypeSectionPublicSupportError<E> {
    Provider,
    Section(Box<CrossConeHirInterfaceSemanticValidationError<E>>),
}
impl<E: std::fmt::Display> std::fmt::Display for TypeSectionPublicSupportError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Provider => f.write_str("type section public support has a different provider"),
            Self::Section(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for TypeSectionPublicSupportError<E> {}
