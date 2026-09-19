use scoop_identity::{
    DecodedCallableTemplateOrigin, DecodedOptionalSignatureType, DecodedPersistentId,
    DecodedSignatureTypeKey, LocalValueSelector, PersistentId, StructuralDefinitionPath,
};
use scoop_wire::{WireError, WireErrorKind, WirePath, encoded_length};

use super::{ResolutionNode, ResourceChildren};
use crate::{
    CanonicalBooleanV1, CanonicalConstValueV1, CanonicalIntegerConstantV1,
    DecodedExportDefinitionSourceV1, DefaultArrayAccessKindV1, DefaultBinaryOperatorV1,
    DefaultBinderRefV1, DefaultForeignCallbackOperationV1, DefaultIntegerDivRemV1,
    DefaultIntegerKindV1, DefaultNoGcIntegerOperationV1, DefaultPrimitiveBinaryKindV1,
    DefaultPrimitiveUnaryKindV1, DefaultUnaryOperatorV1,
};

macro_rules! scalar {
    ($($target:ty),* $(,)?) => { $(
        impl ResolutionNode for $target {
            fn children<'a>(&'a self, _: &mut ResourceChildren<'a, '_>) -> Result<(), WireError> { Ok(()) }
        }
    )* };
}
scalar!(
    u32,
    std::num::NonZeroU32,
    CanonicalBooleanV1,
    CanonicalIntegerConstantV1,
    DecodedCallableTemplateOrigin,
    DefaultArrayAccessKindV1,
    DefaultBinaryOperatorV1,
    DefaultBinderRefV1,
    DefaultForeignCallbackOperationV1,
    DefaultIntegerDivRemV1,
    DefaultIntegerKindV1,
    DefaultNoGcIntegerOperationV1,
    DefaultPrimitiveBinaryKindV1,
    DefaultPrimitiveUnaryKindV1,
    DefaultUnaryOperatorV1
);

impl<I: PersistentId> ResolutionNode for DecodedPersistentId<I> {
    fn children<'a>(&'a self, _: &mut ResourceChildren<'a, '_>) -> Result<(), WireError> {
        Ok(())
    }
}
impl ResolutionNode for String {
    fn children<'a>(&'a self, children: &mut ResourceChildren<'a, '_>) -> Result<(), WireError> {
        children
            .meter
            .check_semantic_leaf(self.len() as u64, &WirePath::root())?;
        children.leaf_bytes(self.len() as u64)
    }
}
impl ResolutionNode for CanonicalConstValueV1 {
    fn children<'a>(&'a self, children: &mut ResourceChildren<'a, '_>) -> Result<(), WireError> {
        match self {
            Self::Integer(value) => children.push(value),
            Self::Boolean(value) => children.push(value),
            Self::String(value) => children.push(value),
        }
    }
}
impl ResolutionNode for DecodedSignatureTypeKey {
    fn children<'a>(&'a self, children: &mut ResourceChildren<'a, '_>) -> Result<(), WireError> {
        for _ in 0..children.copies {
            self.charge_resolution_from_depth(children.meter, children.depth)?;
        }
        Ok(())
    }
}
impl ResolutionNode for DecodedOptionalSignatureType {
    fn children<'a>(&'a self, children: &mut ResourceChildren<'a, '_>) -> Result<(), WireError> {
        match self {
            Self::Absent => Ok(()),
            Self::Present(value) => children.push(value),
        }
    }
}
macro_rules! encoded_leaf {
    ($($target:ty),* $(,)?) => { $(
        impl ResolutionNode for $target {
            fn children<'a>(&'a self, children: &mut ResourceChildren<'a, '_>) -> Result<(), WireError> {
                let bytes = encoded_length(self).map_err(|_| WireError::new(
                    WireErrorKind::IntegerOutOfRange, WirePath::root(), None,
                ))?;
                children.leaf_bytes(bytes)
            }
        }
    )* };
}
encoded_leaf!(DecodedExportDefinitionSourceV1);

impl ResolutionNode for StructuralDefinitionPath {
    fn children<'a>(&'a self, children: &mut ResourceChildren<'a, '_>) -> Result<(), WireError> {
        let count = self.segments().len() as u64;
        let path = WirePath::root();
        for _ in 0..children.copies {
            children.meter.charge_collection_slots(count, &path)?;
            children.meter.charge_nodes(count, &path)?;
            children.meter.charge_edges(count, &path)?;
            children.meter.charge_work(count, &path)?;
        }
        Ok(())
    }
}

impl ResolutionNode for LocalValueSelector {
    fn children<'a>(&'a self, children: &mut ResourceChildren<'a, '_>) -> Result<(), WireError> {
        match self {
            Self::This => Ok(()),
            Self::Parameter { declaration_index } => children.push(declaration_index),
            Self::LocalDeclaration { path }
            | Self::BoundReceiver { path }
            | Self::Synthetic { path, .. } => children.push(path),
            Self::SuspensionResult { site } => children.push(site),
        }
    }
}
