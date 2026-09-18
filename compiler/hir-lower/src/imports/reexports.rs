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
    pub(crate) routes: hir::CanonicalReexportRoutesV1,
    pub(crate) origins: ast::NonEmptyVec<ImportSyntaxOrigin>,
}

impl PartialEq for FrozenReexportBinding {
    fn eq(&self, other: &Self) -> bool {
        self.identity == other.identity
            && self.target == other.target
            && self.routes == other.routes
            && self.origins.iter().eq(other.origins.iter())
    }
}

impl Eq for FrozenReexportBinding {}

struct ReexportAccumulator {
    target: hir::ImportedTarget,
    routes: Vec<hir::ReexportRouteV1>,
    origins: Vec<ImportSyntaxOrigin>,
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

        let mut reexports = Vec::with_capacity(pending.len());
        for (key, mut binding) in pending {
            binding.origins.sort_by(|left, right| {
                left.source
                    .cmp(&right.source)
                    .then_with(|| left.span.start.cmp(&right.span.start))
                    .then_with(|| left.span.end.cmp(&right.span.end))
            });
            binding.origins.dedup();
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
                routes,
                origins: ast::NonEmptyVec::new(first.clone(), rest.to_vec()),
            });
        }
        reexports.sort_unstable_by_key(|binding| binding.identity.id());
        self.reexports = reexports;
        Ok(())
    }
}

fn append(
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
        routes: Vec::new(),
        origins: Vec::new(),
    });
    if entry.target != binding.target() {
        return Err(ReexportPlanBuildError::TargetMismatch);
    }
    entry.routes.extend(
        binding
            .sources()
            .map(|source| source.witness().route().clone()),
    );
    entry.origins.push(origin.clone());
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
    MissingOrigin,
    InvalidIdentity(String),
    InvalidRoutes(hir::ReexportRouteSetBuildError),
}

impl ReexportPlanBuildError {
    pub(crate) const fn file(&self) -> usize {
        match self {
            Self::InvalidName { file, .. } => *file,
            Self::CoreSource
            | Self::InvalidPackage(_)
            | Self::NonDependencyTarget
            | Self::TargetMismatch
            | Self::MissingOrigin
            | Self::InvalidIdentity(_)
            | Self::InvalidRoutes(_) => 0,
        }
    }

    pub(crate) const fn span(&self) -> ast::Span {
        match self {
            Self::InvalidName { span, .. } => *span,
            Self::CoreSource
            | Self::InvalidPackage(_)
            | Self::NonDependencyTarget
            | Self::TargetMismatch
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
