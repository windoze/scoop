use std::collections::BTreeMap;
use std::fmt;

use scoop_identity::{
    CanonicalIdentifier, CanonicalIdentifierError, CborIdentityRecord, ExportBindingKey,
    PackagePath,
};

use super::*;
use crate::{Lowerer, namespace::TopLevelLookupLayer};

#[derive(Debug, Clone)]
pub(crate) struct FrozenReexportBinding {
    pub(crate) identity: hir::HirExportBindingIdentity,
    pub(crate) target: hir::ImportedTarget,
    pub(crate) conflict: hir::ImportedBindingConflictKey,
    pub(crate) routes: hir::CanonicalReexportRoutesV1,
    pub(crate) origins: ast::NonEmptyVec<ImportSyntaxOrigin>,
}

impl PartialEq for FrozenReexportBinding {
    fn eq(&self, other: &Self) -> bool {
        self.identity == other.identity
            && self.target == other.target
            && self.conflict == other.conflict
            && self.routes == other.routes
            && self.origins.iter().eq(other.origins.iter())
    }
}

impl Eq for FrozenReexportBinding {}

struct ReexportAccumulator {
    target: hir::ImportedTarget,
    conflict: hir::ImportedBindingConflictKey,
    routes: Vec<hir::ReexportRouteV1>,
    origins: Vec<PendingReexportOrigin>,
}

#[derive(Clone)]
struct PendingReexportOrigin {
    file: usize,
    syntax: ImportSyntaxOrigin,
}

impl CurrentUnitImports {
    pub(crate) fn freeze_reexports(
        &mut self,
        lowerer: &Lowerer,
    ) -> Result<(), ReexportPlanBuildError> {
        let mut pending = BTreeMap::<ExportBindingKey, ReexportAccumulator>::new();
        for (file, imports) in self.files.iter().enumerate() {
            let has_reexports = imports
                .exact
                .iter()
                .any(|import| import.exposure == ResolvedImportExposure::PublicReexport)
                || imports
                    .stars
                    .iter()
                    .any(|import| import.exposure == ResolvedImportExposure::PublicReexport);
            if !has_reexports {
                continue;
            }
            let package = package(lowerer, file)?;
            for import in &imports.exact {
                if import.exposure != ResolvedImportExposure::PublicReexport {
                    continue;
                }
                let name = canonical_name(file, import.origin.span, &import.local_name)?;
                for target in import.targets.iter() {
                    append(
                        file,
                        lowerer.current_cone(),
                        package.clone(),
                        name.clone(),
                        target,
                        &import.origin,
                        &mut pending,
                    )?;
                }
            }
            for import in &imports.stars {
                if import.exposure != ResolvedImportExposure::PublicReexport {
                    continue;
                }
                for (name, targets) in &import.snapshot {
                    let name = canonical_name(file, import.origin.span, name)?;
                    for target in targets.iter() {
                        append(
                            file,
                            lowerer.current_cone(),
                            package.clone(),
                            name.clone(),
                            target,
                            &import.origin,
                            &mut pending,
                        )?;
                    }
                }
            }
        }

        for binding in pending.values_mut() {
            binding.origins.sort_by(|left, right| {
                left.syntax
                    .source
                    .cmp(&right.syntax.source)
                    .then_with(|| left.syntax.span.start.cmp(&right.syntax.span.start))
                    .then_with(|| left.syntax.span.end.cmp(&right.syntax.span.end))
            });
            binding
                .origins
                .dedup_by(|left, right| left.syntax == right.syntax);
        }
        validate_destination_conflicts(&pending)?;

        let mut reexports = Vec::with_capacity(pending.len());
        for (key, binding) in pending {
            let [first, rest @ ..] = binding.origins.as_slice() else {
                return Err(ReexportPlanBuildError::MissingOrigin);
            };
            let identity = CborIdentityRecord::from_key(key)
                .map_err(|source| ReexportPlanBuildError::InvalidIdentity(source.to_string()))?;
            let routes = hir::CanonicalReexportRoutesV1::try_new(binding.routes)
                .map_err(ReexportPlanBuildError::InvalidRoutes)?;
            reexports.push(FrozenReexportBinding {
                identity,
                target: binding.target,
                conflict: binding.conflict,
                routes,
                origins: ast::NonEmptyVec::new(
                    first.syntax.clone(),
                    rest.iter().map(|origin| origin.syntax.clone()).collect(),
                ),
            });
        }
        reexports.sort_unstable_by_key(|binding| binding.identity.id());
        self.reexports = reexports;
        Ok(())
    }
}

fn append(
    file: usize,
    exporter: scoop_identity::ConeIdentity,
    package: PackagePath,
    name: CanonicalIdentifier,
    target: &ImportedTargetBinding,
    origin: &ImportSyntaxOrigin,
    pending: &mut BTreeMap<ExportBindingKey, ReexportAccumulator>,
) -> Result<(), ReexportPlanBuildError> {
    let ImportedTargetBinding::DirectDependency(binding) = target else {
        return Err(ReexportPlanBuildError::NonDependencyTarget);
    };
    if binding.binding_target().target() != binding.target().persistent() {
        return Err(ReexportPlanBuildError::TargetMismatch);
    }
    let key = ExportBindingKey::new(exporter, package, name, binding.binding_target());
    let entry = pending.entry(key).or_insert_with(|| ReexportAccumulator {
        target: binding.target(),
        conflict: binding.conflict_key().clone(),
        routes: Vec::new(),
        origins: Vec::new(),
    });
    if entry.target != binding.target() {
        return Err(ReexportPlanBuildError::TargetMismatch);
    }
    if entry.conflict != *binding.conflict_key() {
        return Err(ReexportPlanBuildError::ConflictKeyMismatch);
    }
    entry.routes.extend(
        binding
            .sources()
            .map(|source| source.witness().route().clone()),
    );
    entry.origins.push(PendingReexportOrigin {
        file,
        syntax: origin.clone(),
    });
    Ok(())
}

fn validate_destination_conflicts(
    pending: &BTreeMap<ExportBindingKey, ReexportAccumulator>,
) -> Result<(), ReexportPlanBuildError> {
    let mut occupied = BTreeMap::<
        (
            PackagePath,
            scoop_identity::BindingNamespace,
            CanonicalIdentifier,
        ),
        Vec<(
            scoop_identity::BindingTarget,
            hir::ImportedBindingConflictKey,
        )>,
    >::new();
    for (key, binding) in pending {
        let slot = (key.package().clone(), key.namespace(), key.name().clone());
        let targets = occupied.entry(slot).or_default();
        if targets.iter().any(|(target, conflict)| {
            *target != key.binding_target() && conflict.conflicts_with(&binding.conflict)
        }) {
            let origin = binding
                .origins
                .first()
                .ok_or(ReexportPlanBuildError::MissingOrigin)?;
            return Err(ReexportPlanBuildError::DestinationConflict {
                file: origin.file,
                span: origin.syntax.span,
            });
        }
        if !targets
            .iter()
            .any(|(target, _)| *target == key.binding_target())
        {
            targets.push((key.binding_target(), binding.conflict.clone()));
        }
    }
    Ok(())
}

fn package(lowerer: &Lowerer, file: usize) -> Result<PackagePath, ReexportPlanBuildError> {
    let TopLevelLookupLayer::CurrentPackage(package) =
        lowerer.top_level_namespaces.source_namespace(file)
    else {
        return Err(ReexportPlanBuildError::CoreSource);
    };
    lowerer
        .top_level_namespaces
        .package_segments(package)
        .into_iter()
        .map(CanonicalIdentifier::new)
        .collect::<Result<Vec<_>, _>>()
        .map(PackagePath::from_segments)
        .map_err(ReexportPlanBuildError::InvalidPackage)
}

fn canonical_name(
    file: usize,
    span: ast::Span,
    name: &str,
) -> Result<CanonicalIdentifier, ReexportPlanBuildError> {
    CanonicalIdentifier::new(name).map_err(|source| ReexportPlanBuildError::InvalidName {
        file,
        span,
        source,
    })
}

#[derive(Debug)]
pub(crate) enum ReexportPlanBuildError {
    CoreSource,
    InvalidPackage(CanonicalIdentifierError),
    InvalidName {
        file: usize,
        span: ast::Span,
        source: CanonicalIdentifierError,
    },
    NonDependencyTarget,
    TargetMismatch,
    ConflictKeyMismatch,
    DestinationConflict {
        file: usize,
        span: ast::Span,
    },
    MissingOrigin,
    InvalidIdentity(String),
    InvalidRoutes(hir::ReexportRouteSetBuildError),
}

impl ReexportPlanBuildError {
    pub(crate) const fn file(&self) -> usize {
        match self {
            Self::InvalidName { file, .. } => *file,
            Self::DestinationConflict { file, .. } => *file,
            Self::CoreSource
            | Self::InvalidPackage(_)
            | Self::NonDependencyTarget
            | Self::TargetMismatch
            | Self::ConflictKeyMismatch
            | Self::MissingOrigin
            | Self::InvalidIdentity(_)
            | Self::InvalidRoutes(_) => 0,
        }
    }

    pub(crate) const fn span(&self) -> ast::Span {
        match self {
            Self::InvalidName { span, .. } => *span,
            Self::DestinationConflict { span, .. } => *span,
            Self::CoreSource
            | Self::InvalidPackage(_)
            | Self::NonDependencyTarget
            | Self::TargetMismatch
            | Self::ConflictKeyMismatch
            | Self::MissingOrigin
            | Self::InvalidIdentity(_)
            | Self::InvalidRoutes(_) => ast::Span { start: 0, end: 0 },
        }
    }
}

impl fmt::Display for ReexportPlanBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CoreSource => formatter.write_str("a core source cannot declare a re-export"),
            Self::InvalidPackage(source) => {
                write!(formatter, "invalid re-export package: {source}")
            }
            Self::InvalidName { source, .. } => {
                write!(formatter, "invalid re-export binding name: {source}")
            }
            Self::NonDependencyTarget => {
                formatter.write_str("a public re-export must target a direct dependency binding")
            }
            Self::TargetMismatch => formatter.write_str(
                "a re-export binding target disagrees with its imported terminal target",
            ),
            Self::ConflictKeyMismatch => formatter
                .write_str("a re-export binding disagrees with its imported target conflict key"),
            Self::DestinationConflict { .. } => {
                formatter.write_str("re-export destination conflicts with another public binding")
            }
            Self::MissingOrigin => {
                formatter.write_str("a re-export binding must retain a source origin")
            }
            Self::InvalidIdentity(source) => {
                write!(
                    formatter,
                    "cannot derive re-export binding identity: {source}"
                )
            }
            Self::InvalidRoutes(source) => write!(formatter, "invalid re-export routes: {source}"),
        }
    }
}

impl std::error::Error for ReexportPlanBuildError {}
