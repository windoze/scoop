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
    ExportBinding(scoop_identity::PersistentExportBindingId),
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
    pub exports: Vec<scoop_identity::PersistentExportBindingId>,
}

/// Successful exact-layer lookup; targets remain the ordinary bound targets.
#[derive(Clone, Debug)]
pub struct EditingImportUse {
    pub span: crate::Span,
    pub imports: Vec<crate::EditingImportSource>,
}

/// An index of completed observations, with unavailable analysis kept explicit.
#[derive(Clone, Debug)]
pub struct EditingReferenceIndex {
    pub definitions: HashMap<EditingReferenceTarget, SemanticSourceLocation>,
    occurrences: Vec<EditingReference>,
    targets: HashMap<EditingReferenceTarget, Vec<usize>>,
    locations: HashMap<SemanticSourceLocation, Vec<EditingReferenceTarget>>,
    pub unavailable: Vec<SemanticSourceLocation>,
    pub declarations_available: bool,
    pub aliases: la_arena::Arena<EditingImportAlias>,
    pub export_references: Vec<crate::EditingExportReference>,
    pub reexports: Vec<crate::EditingReexport>,
}

impl EditingReferenceIndex {
    pub fn new(
        occurrences: Vec<EditingReference>,
        definitions: HashMap<EditingReferenceTarget, SemanticSourceLocation>,
        unavailable: Vec<SemanticSourceLocation>,
        declarations_available: bool,
        aliases: la_arena::Arena<EditingImportAlias>,
        export_references: Vec<crate::EditingExportReference>,
        reexports: Vec<crate::EditingReexport>,
    ) -> Self {
        let exported = export_references.iter().flat_map(|reference| {
            reference.routes.iter().flat_map(move |route| {
                route.hops().iter().map(move |hop| EditingReference {
                    target: EditingReferenceTarget::ExportBinding(hop.binding()),
                    origin: reference.origin.clone(),
                    is_declaration: false,
                    aliases: reference.aliases.clone(),
                })
            })
        });
        let mut seen = HashSet::new();
        let occurrences = occurrences
            .into_iter()
            .chain(exported)
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
        for reference in &export_references {
            let Some(at) = locations.get_mut(&reference.origin) else {
                continue;
            };
            if reference.routes.iter().all(|route| route.hops().len() > 1) {
                at.retain(|target| *target != reference.target);
            }
            let heads = reference
                .routes
                .iter()
                .filter(|route| route.hops().len() > 1)
                .map(|route| route.hops()[0].binding())
                .collect::<HashSet<_>>();
            let bindings = reference
                .routes
                .iter()
                .flat_map(|route| route.hops())
                .map(|hop| hop.binding())
                .collect::<HashSet<_>>();
            at.retain(|target| match target {
                EditingReferenceTarget::ExportBinding(binding) => {
                    !bindings.contains(binding) || heads.contains(binding)
                }
                _ => true,
            });
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
            if at.iter().any(|target| {
                matches!(
                    target,
                    EditingReferenceTarget::Callable(EditingCallableIdentity::Constructor(_))
                )
            }) {
                let named_alias = at.iter().any(|target| {
                    matches!(
                        target,
                        EditingReferenceTarget::Type(crate::EditingTypeIdentity::Alias(_))
                            | EditingReferenceTarget::ExportBinding(_)
                    )
                });
                at.retain(|target| {
                    if named_alias {
                        !matches!(
                            target,
                            EditingReferenceTarget::Callable(EditingCallableIdentity::Constructor(
                                _
                            ))
                        )
                    } else {
                        !matches!(
                            target,
                            EditingReferenceTarget::Type(crate::EditingTypeIdentity::Nominal(_))
                        )
                    }
                });
            }
        }
        Self {
            definitions,
            occurrences,
            targets,
            locations,
            unavailable,
            declarations_available,
            aliases,
            export_references,
            reexports,
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
