use super::*;

#[derive(Debug)]
pub enum SharedLirPhysicalError {
    Resource(scoop_wire::WireError),
    Layout(Box<lir::LayoutAbiSectionError<Infallible>>),
    Contract(Box<lir::ShapeLinkError>),
    Strong(Box<lir::StrongProductionLayoutJoinError>),
    LinkImports(Box<crate::LayoutLinkClosureError>),
    LinkMaterializations(Box<crate::StrongLinkMaterializationError>),
    LinkObjectContents(Box<crate::LayoutLinkObjectContentsError>),
}

macro_rules! from_error {
    ($source:ty, $variant:ident) => {
        impl From<$source> for SharedLirPhysicalError {
            fn from(value: $source) -> Self {
                Self::$variant(Box::new(value))
            }
        }
    };
}
from_error!(lir::LayoutAbiSectionError<Infallible>, Layout);
from_error!(lir::ShapeLinkError, Contract);
from_error!(lir::StrongProductionLayoutJoinError, Strong);
from_error!(crate::LayoutLinkClosureError, LinkImports);
from_error!(crate::StrongLinkMaterializationError, LinkMaterializations);
from_error!(crate::LayoutLinkObjectContentsError, LinkObjectContents);
impl From<scoop_wire::WireError> for SharedLirPhysicalError {
    fn from(value: scoop_wire::WireError) -> Self {
        Self::Resource(value)
    }
}

#[derive(Debug)]
pub struct CrossConeLayoutLirPhysicalError {
    pub provider: ConeIdentity,
    pub source: Box<SharedLirPhysicalError>,
}
impl std::fmt::Display for CrossConeLayoutLirPhysicalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid shared LIR physical imports for {:?}: {:?}",
            self.provider, self.source
        )
    }
}
impl std::error::Error for CrossConeLayoutLirPhysicalError {}
