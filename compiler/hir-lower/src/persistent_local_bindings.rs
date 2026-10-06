use std::fmt;

use scoop_ast::Span;
use scoop_hir as hir;
use scoop_identity::{
    BindingTarget, BindingTargetError, CanonicalIdentifier, CanonicalIdentifierError,
    DefinitionOrigin, LocalBindingKey, LocalBindingRole, PackagePath, SourceIdentity,
    SourceOriginError, SourceSpan, SourceSpanError,
};

use crate::{Lowerer, imports::CurrentUnitTarget, namespace::TopLevelLookupLayer};

#[allow(clippy::too_many_arguments)]
pub(crate) fn build(
    lowerer: &Lowerer,
    nominals: &hir::HirNominalIdentities,
    object_values: &hir::HirObjectValueIdentities,
    functions: &hir::HirFunctionIdentities,
    properties: &hir::HirPropertyIdentities,
    aliases: &hir::HirTypeAliasIdentities,
    enum_members: &hir::HirEnumMemberIdentities,
    source_contexts: &hir::HirSourceContextIdentities,
) -> Result<hir::HirLocalBindingIdentities, PersistentLocalBindingIdentityError> {
    let tables = IdentityTables {
        nominals,
        object_values,
        functions,
        properties,
        aliases,
        enum_members,
    };
    let mut identities = Vec::new();

    for binding in &lowerer.imports.bindings {
        append_binding(
            lowerer,
            &tables,
            source_contexts,
            &mut identities,
            binding.source.clone(),
            binding.file,
            binding.span,
            &binding.name,
            binding.target,
            LocalBindingRole::Declaration,
        )?;
    }

    for (file, imports) in lowerer.imports.files.iter().enumerate() {
        for import in &imports.exact {
            for imported in import.targets.iter() {
                append_imported_binding(
                    lowerer,
                    &tables,
                    source_contexts,
                    &mut identities,
                    import.origin.source.clone(),
                    file,
                    import.origin.span,
                    &import.local_name,
                    imported,
                    import.source_role,
                )?;
            }
        }
        for import in &imports.stars {
            for (name, imported_bindings) in &import.snapshot {
                for imported in imported_bindings.iter() {
                    append_imported_binding(
                        lowerer,
                        &tables,
                        source_contexts,
                        &mut identities,
                        import.origin.source.clone(),
                        file,
                        import.origin.span,
                        name,
                        imported,
                        LocalBindingRole::StarImport,
                    )?;
                }
            }
        }
    }

    hir::HirLocalBindingIdentities::canonicalize(identities).map_err(|detail| {
        PersistentLocalBindingIdentityError {
            file: 0,
            span: Span { start: 0, end: 0 },
            detail: PersistentLocalBindingIdentityErrorDetail::Identity(detail),
        }
    })
}

#[allow(clippy::too_many_arguments)]
fn append_imported_binding(
    lowerer: &Lowerer,
    tables: &IdentityTables<'_>,
    source_contexts: &hir::HirSourceContextIdentities,
    identities: &mut Vec<hir::HirLocalBindingIdentity>,
    source: SourceIdentity,
    file: usize,
    span: Span,
    local_name: &str,
    imported: &crate::imports::ImportedTargetBinding,
    source_role: LocalBindingRole,
) -> Result<(), PersistentLocalBindingIdentityError> {
    match imported {
        crate::imports::ImportedTargetBinding::CurrentCone { binding, .. } => append_binding(
            lowerer,
            tables,
            source_contexts,
            identities,
            source,
            file,
            span,
            local_name,
            lowerer.imports.binding(*binding).target,
            source_role,
        ),
        crate::imports::ImportedTargetBinding::DirectDependency(binding) => append_binding_targets(
            lowerer,
            source_contexts,
            identities,
            source,
            file,
            span,
            local_name,
            std::iter::once(binding.binding_target()),
            source_role,
        ),
    }
}

struct IdentityTables<'a> {
    nominals: &'a hir::HirNominalIdentities,
    object_values: &'a hir::HirObjectValueIdentities,
    functions: &'a hir::HirFunctionIdentities,
    properties: &'a hir::HirPropertyIdentities,
    aliases: &'a hir::HirTypeAliasIdentities,
    enum_members: &'a hir::HirEnumMemberIdentities,
}

#[allow(clippy::too_many_arguments)]
fn append_binding(
    lowerer: &Lowerer,
    tables: &IdentityTables<'_>,
    source_contexts: &hir::HirSourceContextIdentities,
    identities: &mut Vec<hir::HirLocalBindingIdentity>,
    source: SourceIdentity,
    file: usize,
    span: Span,
    local_name: &str,
    target: CurrentUnitTarget,
    source_role: LocalBindingRole,
) -> Result<(), PersistentLocalBindingIdentityError> {
    let targets =
        binding_targets(lowerer, tables, target).map_err(|detail| failure(file, span, detail))?;

    append_binding_targets(
        lowerer,
        source_contexts,
        identities,
        source,
        file,
        span,
        local_name,
        targets,
        source_role,
    )
}

#[allow(clippy::too_many_arguments)]
fn append_binding_targets(
    lowerer: &Lowerer,
    source_contexts: &hir::HirSourceContextIdentities,
    identities: &mut Vec<hir::HirLocalBindingIdentity>,
    source: SourceIdentity,
    file: usize,
    span: Span,
    local_name: &str,
    targets: impl IntoIterator<Item = BindingTarget>,
    source_role: LocalBindingRole,
) -> Result<(), PersistentLocalBindingIdentityError> {
    let package = package(lowerer, file).map_err(|detail| failure(file, span, detail))?;
    let local_name = CanonicalIdentifier::new(local_name).map_err(|error| {
        failure(
            file,
            span,
            PersistentLocalBindingIdentityErrorDetail::InvalidName(error),
        )
    })?;
    let origin = definition_origin(lowerer, source_contexts, &source, file, span)
        .map_err(|detail| failure(file, span, detail))?;

    for target in targets {
        let key = LocalBindingKey::new(
            source.clone(),
            package.clone(),
            local_name.clone(),
            target,
            source_role,
        );
        identities.push(
            hir::HirLocalBindingIdentity::new(key, origin.clone()).map_err(|error| {
                failure(
                    file,
                    span,
                    PersistentLocalBindingIdentityErrorDetail::Identity(error),
                )
            })?,
        );
    }
    Ok(())
}

fn package(
    lowerer: &Lowerer,
    file: usize,
) -> Result<PackagePath, PersistentLocalBindingIdentityErrorDetail> {
    let TopLevelLookupLayer::CurrentPackage(package) =
        lowerer.top_level_namespaces.source_namespace(file)
    else {
        return Err(PersistentLocalBindingIdentityErrorDetail::CoreBinding);
    };
    let segments = lowerer
        .top_level_namespaces
        .package_segments(package)
        .into_iter()
        .map(CanonicalIdentifier::new)
        .collect::<Result<Vec<_>, _>>()
        .map_err(PersistentLocalBindingIdentityErrorDetail::InvalidName)?;
    Ok(PackagePath::from_segments(segments))
}

fn definition_origin(
    lowerer: &Lowerer,
    source_contexts: &hir::HirSourceContextIdentities,
    source: &SourceIdentity,
    file: usize,
    span: Span,
) -> Result<DefinitionOrigin, PersistentLocalBindingIdentityErrorDetail> {
    if &lowerer.visibility_file(file) != source {
        return Err(PersistentLocalBindingIdentityErrorDetail::BindingSourceMismatch);
    }
    let span = SourceSpan::new(u64::from(span.start), u64::from(span.end))
        .map_err(PersistentLocalBindingIdentityErrorDetail::InvalidSpan)?;
    DefinitionOrigin::new(
        source.clone(),
        span,
        source_contexts[lowerer.file_source_contexts[file]].key(),
    )
    .map_err(PersistentLocalBindingIdentityErrorDetail::InvalidOrigin)
}

fn binding_targets(
    lowerer: &Lowerer,
    tables: &IdentityTables<'_>,
    target: CurrentUnitTarget,
) -> Result<Vec<BindingTarget>, PersistentLocalBindingIdentityErrorDetail> {
    let targets = match target {
        CurrentUnitTarget::Class(id) => vec![type_name(tables.nominals[id].source())?],
        CurrentUnitTarget::Interface(id) => vec![type_name(tables.nominals[id].source())?],
        CurrentUnitTarget::Struct(id) => vec![type_name(tables.nominals[id].source())?],
        CurrentUnitTarget::Enum(id) => vec![type_name(tables.nominals[id].source())?],
        CurrentUnitTarget::Object(id) => {
            let nominal = tables.nominals[id]
                .source()
                .ok_or(PersistentLocalBindingIdentityErrorDetail::GeneratedNominal)?;
            let value = &tables.object_values[lowerer.objects[id].singleton_value];
            vec![
                BindingTarget::type_name(nominal.declaration()),
                BindingTarget::object_value(value.record().key()),
            ]
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(PersistentLocalBindingIdentityErrorDetail::InvalidTarget)?
        }
        CurrentUnitTarget::Annotation(id) => vec![
            BindingTarget::annotation(lowerer.source_annotations[&id].identity.key())
                .map_err(PersistentLocalBindingIdentityErrorDetail::InvalidTarget)?,
        ],
        CurrentUnitTarget::TypeAlias(id) => {
            let declaration = lowerer
                .published_type_alias(id)
                .ok_or(PersistentLocalBindingIdentityErrorDetail::UnpublishedTypeAlias)?;
            vec![
                BindingTarget::type_alias(tables.aliases[declaration].key())
                    .map_err(PersistentLocalBindingIdentityErrorDetail::InvalidTarget)?,
            ]
        }
        CurrentUnitTarget::Function(id) => {
            let identity = tables.functions[id]
                .source_identity()
                .ok_or(PersistentLocalBindingIdentityErrorDetail::GeneratedFunction)?;
            let declaration = identity.declaration();
            let target = if declaration.duplicate_signature().receiver_is_present() {
                BindingTarget::extension_function(declaration)
            } else {
                BindingTarget::function(declaration)
            }
            .map_err(PersistentLocalBindingIdentityErrorDetail::InvalidTarget)?;
            vec![target]
        }
        CurrentUnitTarget::Property(id) => {
            let identity = &tables.properties[id];
            let target = match identity {
                hir::HirPropertyIdentity::Ordinary(_) => {
                    BindingTarget::property(identity.declaration())
                }
                hir::HirPropertyIdentity::Extension(_) => {
                    BindingTarget::extension_property(identity.declaration())
                }
            }
            .map_err(PersistentLocalBindingIdentityErrorDetail::InvalidTarget)?;
            vec![target]
        }
        CurrentUnitTarget::EnumVariant(variant) => {
            vec![BindingTarget::enum_variant(
                tables.enum_members[variant].id(),
            )]
        }
        CurrentUnitTarget::SourceProperty(_) => {
            return Err(PersistentLocalBindingIdentityErrorDetail::UnresolvedProperty);
        }
        CurrentUnitTarget::SourceVariant(_) => {
            return Err(PersistentLocalBindingIdentityErrorDetail::UnresolvedVariant);
        }
    };
    Ok(targets)
}

fn type_name(
    identity: Option<&hir::HirSourceNominalIdentity>,
) -> Result<BindingTarget, PersistentLocalBindingIdentityErrorDetail> {
    let identity = identity.ok_or(PersistentLocalBindingIdentityErrorDetail::GeneratedNominal)?;
    BindingTarget::type_name(identity.declaration())
        .map_err(PersistentLocalBindingIdentityErrorDetail::InvalidTarget)
}

#[derive(Debug)]
pub(crate) struct PersistentLocalBindingIdentityError {
    file: usize,
    span: Span,
    detail: PersistentLocalBindingIdentityErrorDetail,
}

impl PersistentLocalBindingIdentityError {
    pub(crate) const fn file(&self) -> usize {
        self.file
    }

    pub(crate) const fn span(&self) -> Span {
        self.span
    }
}

impl fmt::Display for PersistentLocalBindingIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot derive persistent local binding identity: {}",
            self.detail
        )
    }
}

impl std::error::Error for PersistentLocalBindingIdentityError {}

#[derive(Debug)]
enum PersistentLocalBindingIdentityErrorDetail {
    CoreBinding,
    BindingSourceMismatch,
    GeneratedNominal,
    GeneratedFunction,
    UnpublishedTypeAlias,
    UnresolvedProperty,
    UnresolvedVariant,
    InvalidName(CanonicalIdentifierError),
    InvalidSpan(SourceSpanError),
    InvalidOrigin(SourceOriginError),
    InvalidTarget(BindingTargetError),
    Identity(hir::HirLocalBindingIdentityError),
}

impl fmt::Display for PersistentLocalBindingIdentityErrorDetail {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CoreBinding => formatter.write_str("core bindings are outside the current Cone"),
            Self::BindingSourceMismatch => {
                formatter.write_str("binding source does not match its source file")
            }
            Self::GeneratedNominal => {
                formatter.write_str("a source binding refers to a generated nominal")
            }
            Self::GeneratedFunction => {
                formatter.write_str("a source binding refers to a generated function")
            }
            Self::UnpublishedTypeAlias => {
                formatter.write_str("a source binding refers to an unpublished type alias")
            }
            Self::UnresolvedProperty => {
                formatter.write_str("a source binding refers to an unresolved property")
            }
            Self::UnresolvedVariant => {
                formatter.write_str("a source binding refers to an unresolved enum variant")
            }
            Self::InvalidName(error) => error.fmt(formatter),
            Self::InvalidSpan(error) => error.fmt(formatter),
            Self::InvalidOrigin(error) => error.fmt(formatter),
            Self::InvalidTarget(error) => error.fmt(formatter),
            Self::Identity(error) => error.fmt(formatter),
        }
    }
}

fn failure(
    file: usize,
    span: Span,
    detail: PersistentLocalBindingIdentityErrorDetail,
) -> PersistentLocalBindingIdentityError {
    PersistentLocalBindingIdentityError { file, span, detail }
}

#[cfg(test)]
mod tests;
