use std::collections::BTreeMap;

use scoop_hir as hir;
use scoop_identity::{
    BindableEntity, BindingNamespace, BindingTarget, CanonicalIdentifier, PackagePath,
};

use super::{PersistentExportBindingIdentityError, PersistentExportBindings};
use crate::{Lowerer, imports::FrozenReexportBinding};

#[derive(Clone, Copy)]
struct SourceLocation {
    file: usize,
    span: scoop_ast::Span,
}

struct Candidate {
    identity: hir::HirExportBindingIdentity,
    conflict: hir::ImportedBindingConflictKey,
    source: hir::ExportBindingSourceV1,
    location: Option<SourceLocation>,
}

type Destination = (PackagePath, BindingNamespace, CanonicalIdentifier);

pub(super) fn merge(
    lowerer: &Lowerer,
    surface: &hir::PublicSemanticSurface,
    functions: &hir::HirFunctionIdentities,
    properties: &hir::HirPropertyIdentities,
    direct: hir::HirExportBindingIdentities,
) -> Result<PersistentExportBindings, PersistentExportBindingIdentityError> {
    let overloads = direct_overload_signatures(surface, functions, properties);
    let mut candidates = Vec::with_capacity(direct.len() + lowerer.imports.reexports.len());
    for identity in direct.iter() {
        let target = identity.key().target();
        candidates.push(Candidate {
            identity: identity.clone(),
            conflict: direct_conflict_key(target, &overloads)?,
            source: hir::ExportBindingSourceV1::DeclaredCurrent {
                declaration: target,
            },
            location: None,
        });
    }
    for binding in &lowerer.imports.reexports {
        candidates.push(reexport_candidate(lowerer, binding)?);
    }

    seal(candidates)
}

fn seal(
    candidates: Vec<Candidate>,
) -> Result<PersistentExportBindings, PersistentExportBindingIdentityError> {
    validate_destinations(&candidates)?;
    let identities = hir::HirExportBindingIdentities::canonicalize(
        candidates
            .iter()
            .map(|candidate| candidate.identity.clone())
            .collect(),
    )
    .map_err(PersistentExportBindingIdentityError::Identities)?;
    let surface = hir::CanonicalPublicExportBindingsV1::try_new(
        candidates
            .into_iter()
            .map(|candidate| {
                hir::PublicExportBindingRecordV1::new(candidate.identity.id(), candidate.source)
            })
            .collect(),
    )
    .map_err(PersistentExportBindingIdentityError::Surface)?;
    Ok(PersistentExportBindings {
        identities,
        surface,
    })
}

fn direct_overload_signatures(
    surface: &hir::PublicSemanticSurface,
    functions: &hir::HirFunctionIdentities,
    properties: &hir::HirPropertyIdentities,
) -> BTreeMap<BindableEntity, hir::ImportedBindingConflictKey> {
    let mut overloads = BTreeMap::new();
    for &function in &surface.functions {
        let Some(identity) = functions[function].source_identity() else {
            continue;
        };
        let target = match identity {
            hir::HirSourceFunctionIdentity::Plain(record) => BindableEntity::Function(record.id()),
            hir::HirSourceFunctionIdentity::Generic(record) => {
                BindableEntity::GenericFunction(record.id())
            }
        };
        overloads.insert(
            target,
            hir::ImportedBindingConflictKey::Overload(
                identity.declaration().duplicate_signature().clone(),
            ),
        );
    }
    for &property in &surface.properties {
        let hir::HirPropertyIdentity::Extension(record) = &properties[property] else {
            continue;
        };
        overloads.insert(
            BindableEntity::ExtensionProperty(record.id()),
            hir::ImportedBindingConflictKey::Overload(record.key().duplicate_signature().clone()),
        );
    }
    overloads
}

fn direct_conflict_key(
    target: BindableEntity,
    overloads: &BTreeMap<BindableEntity, hir::ImportedBindingConflictKey>,
) -> Result<hir::ImportedBindingConflictKey, PersistentExportBindingIdentityError> {
    match target {
        BindableEntity::Type(_) | BindableEntity::GenericType(_) | BindableEntity::TypeAlias(_) => {
            Ok(hir::ImportedBindingConflictKey::Type)
        }
        BindableEntity::ObjectValue(_)
        | BindableEntity::Property(_)
        | BindableEntity::EnumVariant(_) => Ok(hir::ImportedBindingConflictKey::Value),
        BindableEntity::Function(_)
        | BindableEntity::GenericFunction(_)
        | BindableEntity::ExtensionProperty(_) => overloads
            .get(&target)
            .cloned()
            .ok_or(PersistentExportBindingIdentityError::UnknownOverloadSignature { target }),
    }
}

fn reexport_candidate(
    lowerer: &Lowerer,
    binding: &FrozenReexportBinding,
) -> Result<Candidate, PersistentExportBindingIdentityError> {
    let origin = binding.origins.first();
    let file = lowerer
        .intrinsic_sources
        .iter()
        .position(|source| source.identity == origin.source)
        .ok_or_else(
            || PersistentExportBindingIdentityError::UnknownReexportSource {
                source: origin.source.clone(),
                span: origin.span,
            },
        )?;
    let location = SourceLocation {
        file,
        span: origin.span,
    };
    if binding.identity.key().target() != binding.target.persistent()
        || !conflict_key_matches_target(&binding.conflict, binding.identity.key().target())
    {
        return Err(
            PersistentExportBindingIdentityError::InvalidReexportTarget {
                file,
                span: origin.span,
            },
        );
    }
    Ok(Candidate {
        identity: binding.identity.clone(),
        conflict: binding.conflict.clone(),
        source: hir::ExportBindingSourceV1::Reexport {
            routes: binding.routes.clone(),
        },
        location: Some(location),
    })
}

fn conflict_key_matches_target(
    conflict: &hir::ImportedBindingConflictKey,
    target: BindableEntity,
) -> bool {
    matches!(
        (conflict, target),
        (
            hir::ImportedBindingConflictKey::Type,
            BindableEntity::Type(_) | BindableEntity::GenericType(_) | BindableEntity::TypeAlias(_)
        ) | (
            hir::ImportedBindingConflictKey::Value,
            BindableEntity::ObjectValue(_)
                | BindableEntity::Property(_)
                | BindableEntity::EnumVariant(_)
        ) | (
            hir::ImportedBindingConflictKey::Overload(_),
            BindableEntity::Function(_)
                | BindableEntity::GenericFunction(_)
                | BindableEntity::ExtensionProperty(_)
        )
    )
}

fn validate_destinations(
    candidates: &[Candidate],
) -> Result<(), PersistentExportBindingIdentityError> {
    let mut occupied = BTreeMap::<Destination, Vec<(BindingTarget, usize)>>::new();
    for (index, candidate) in candidates.iter().enumerate() {
        let key = candidate.identity.key();
        let target = key.binding_target();
        let destination = (key.package().clone(), key.namespace(), key.name().clone());
        let entries = occupied.entry(destination).or_default();
        for (existing_target, existing_index) in entries.iter() {
            let existing = &candidates[*existing_index];
            if *existing_target == target {
                return Err(at_candidate(candidate, existing, |file, span| {
                    PersistentExportBindingIdentityError::DuplicateBindingSource { file, span }
                }));
            }
            if existing.conflict.conflicts_with(&candidate.conflict) {
                return Err(at_candidate(candidate, existing, |file, span| {
                    PersistentExportBindingIdentityError::DestinationConflict { file, span }
                }));
            }
        }
        entries.push((target, index));
    }
    Ok(())
}

fn at_candidate(
    incoming: &Candidate,
    existing: &Candidate,
    make: impl FnOnce(usize, scoop_ast::Span) -> PersistentExportBindingIdentityError,
) -> PersistentExportBindingIdentityError {
    let location = incoming
        .location
        .or(existing.location)
        .unwrap_or(SourceLocation {
            file: 0,
            span: scoop_ast::Span { start: 0, end: 0 },
        });
    make(location.file, location.span)
}

#[cfg(test)]
mod tests;
