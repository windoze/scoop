use super::{
    AnnotationDataBuildError, AnnotationParameterType, AnnotationTargetV1, CanonicalAnnotationsV1,
};
use crate::{AnnotationTypeSlot, AnnotationTypeUseId, TypeAnnotationNode};
use scoop_identity::SignatureTypeKey;

mod wire;
pub(super) use wire::{DecodedAnnotatedTypeUseV1, DecodedAnnotationTypeSiteV1};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnnotatedTypeUseV1 {
    pub identity: AnnotationTypeUseId,
    pub ty: AnnotationParameterType<SignatureTypeKey>,
    /// Type uses flatten the surrounding declaration's complete binder list.
    pub binder_count: u32,
    pub nodes: Vec<TypeAnnotationNode>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnnotationTypeSiteV1 {
    pub target: AnnotationTargetV1,
    pub slot: AnnotationTypeSlot,
    pub type_use: AnnotationTypeUseId,
}

impl CanonicalAnnotationsV1 {
    pub fn type_uses(&self) -> &[AnnotatedTypeUseV1] {
        &self.type_uses
    }
    pub fn type_sites(&self) -> &[AnnotationTypeSiteV1] {
        &self.type_sites
    }

    pub fn type_use(&self, identity: &AnnotationTypeUseId) -> Option<&AnnotatedTypeUseV1> {
        self.type_uses
            .binary_search_by(|entry| entry.identity.cmp(identity))
            .ok()
            .map(|i| &self.type_uses[i])
    }

    pub fn annotated_type(
        &self,
        target: AnnotationTargetV1,
        slot: AnnotationTypeSlot,
    ) -> Option<&AnnotatedTypeUseV1> {
        let index = self
            .type_sites
            .binary_search_by_key(&(target, slot), |entry| (entry.target, entry.slot))
            .ok()?;
        self.type_use(&self.type_sites[index].type_use)
    }

    pub(super) fn check_type_uses(&self) -> Result<(), AnnotationDataBuildError> {
        if self
            .type_uses
            .windows(2)
            .any(|pair| pair[0].identity == pair[1].identity)
            || self
                .type_sites
                .windows(2)
                .any(|pair| (pair[0].target, pair[0].slot) == (pair[1].target, pair[1].slot))
        {
            return Err(AnnotationDataBuildError(
                "duplicate annotated type use or declaration site",
            ));
        }
        for usage in &self.type_uses {
            if usage.nodes.is_empty()
                || usage.nodes.iter().any(|node| node.annotations.is_empty())
                || usage
                    .nodes
                    .windows(2)
                    .any(|pair| pair[0].path >= pair[1].path)
            {
                return Err(AnnotationDataBuildError(
                    "type annotation paths must be nonempty and in canonical order",
                ));
            }
        }
        if self
            .type_sites
            .iter()
            .any(|site| self.type_use(&site.type_use).is_none())
        {
            return Err(AnnotationDataBuildError(
                "type annotation site has no type use",
            ));
        }
        Ok(())
    }
}
