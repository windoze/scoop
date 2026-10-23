use std::collections::{HashMap, HashSet};

use crate::{
    BindingId, DefaultCallableDeclarationV1, LocalFunctionId, PropertyDeclarationId,
    SemanticSourceLocation,
};

/// Local callable ids belong to the same analysis arena as local bindings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EditingCallableIdentity {
    Declaration(DefaultCallableDeclarationV1),
    Constructor(scoop_identity::PersistentConstructorId),
    Variant(scoop_identity::PersistentEnumVariantId),
    Local(LocalFunctionId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EditingReferenceTarget {
    Binding(BindingId),
    Callable(EditingCallableIdentity),
    Parameter {
        callable: EditingCallableIdentity,
        index: u32,
    },
    Property(PropertyDeclarationId),
    Type(crate::EditingTypeIdentity),
    ImportAlias(EditingImportAliasId),
}

impl EditingReferenceTarget {
    pub const fn is_local(self) -> bool {
        matches!(
            self,
            Self::Binding(_)
                | Self::Type(crate::EditingTypeIdentity::Parameter(_))
                | Self::ImportAlias(_)
                | Self::Callable(EditingCallableIdentity::Local(_))
                | Self::Parameter {
                    callable: EditingCallableIdentity::Local(_),
                    ..
                }
        )
    }
}

#[derive(Clone, Debug)]
pub struct EditingReference {
    pub target: EditingReferenceTarget,
    pub origin: SemanticSourceLocation,
    pub is_declaration: bool,
    /// Explicit aliases through which this occurrence names its target.
    pub aliases: Vec<SemanticSourceLocation>,
}

pub type EditingImportAliasId = la_arena::Idx<EditingImportAlias>;

/// One explicit alias in this Cone's current reference index.
#[derive(Clone, Debug)]
pub struct EditingImportAlias {
    pub name: String,
    pub origin: SemanticSourceLocation,
    pub targets: crate::NonEmptyVec<EditingReferenceTarget>,
    pub reexported: bool,
}

/// Successful exact-layer lookup; targets remain the ordinary bound targets.
#[derive(Clone, Debug)]
pub struct EditingImportUse {
    pub span: crate::Span,
    pub imports: Vec<SemanticSourceLocation>,
}

/// An index of completed observations, with unavailable analysis kept explicit.
#[derive(Clone, Debug)]
pub struct EditingReferenceIndex {
    occurrences: Vec<EditingReference>,
    targets: HashMap<EditingReferenceTarget, Vec<usize>>,
    locations: HashMap<SemanticSourceLocation, Vec<EditingReferenceTarget>>,
    pub unavailable: Vec<SemanticSourceLocation>,
    pub declarations_available: bool,
    pub aliases: la_arena::Arena<EditingImportAlias>,
}

impl EditingReferenceIndex {
    pub fn new(
        occurrences: Vec<EditingReference>,
        unavailable: Vec<SemanticSourceLocation>,
        declarations_available: bool,
        aliases: la_arena::Arena<EditingImportAlias>,
    ) -> Self {
        let mut seen = HashSet::new();
        let occurrences = occurrences
            .into_iter()
            .filter(|occurrence| {
                seen.insert((
                    occurrence.target,
                    occurrence.origin.clone(),
                    occurrence.is_declaration,
                ))
            })
            .collect::<Vec<_>>();
        let mut targets = HashMap::<_, Vec<_>>::new();
        let mut locations = HashMap::<_, Vec<_>>::new();
        for (index, occurrence) in occurrences.iter().enumerate() {
            targets.entry(occurrence.target).or_default().push(index);
            let at = locations.entry(occurrence.origin.clone()).or_default();
            if !at.contains(&occurrence.target) {
                at.push(occurrence.target);
            }
        }
        for at in locations.values_mut() {
            let aliased = at
                .iter()
                .filter_map(|target| match target {
                    EditingReferenceTarget::ImportAlias(alias) => Some(&aliases[*alias].targets),
                    _ => None,
                })
                .flat_map(|targets| targets.iter().copied())
                .collect::<HashSet<_>>();
            if !aliased.is_empty() {
                at.retain(|target| !aliased.contains(target));
            }
        }
        Self {
            occurrences,
            targets,
            locations,
            unavailable,
            declarations_available,
            aliases,
        }
    }

    pub fn targets_at(&self, origin: &SemanticSourceLocation) -> &[EditingReferenceTarget] {
        self.locations.get(origin).map_or(&[], Vec::as_slice)
    }

    pub fn references_to(
        &self,
        target: EditingReferenceTarget,
    ) -> impl Iterator<Item = &EditingReference> {
        self.targets
            .get(&target)
            .into_iter()
            .flatten()
            .map(|index| &self.occurrences[*index])
    }

    pub fn occurrences(&self) -> &[EditingReference] {
        &self.occurrences
    }
}
