//! Project complete source HIR once; subsequent renders consume only this data.

use crate::model::*;
use scoop_hir as hir;
use scoop_identity::{ConeCoordinate, RequestedConeKind};
use std::collections::HashMap;

mod annotations;
mod bindings;
mod comments;
mod keys;
mod relations;
mod requirements;
mod signatures;
mod visibility;

type Target = hir::DocumentationTarget;
type Source = hir::SourceAnnotationTarget;
type Callable = hir::SourceAnnotationCallable;
type Result<T> = std::result::Result<T, ProjectionError>;

#[derive(Debug)]
pub enum ProjectionError {
    Identity(scoop_wire::HashError),
    Annotation(hir::AnnotationProductionError),
}

impl std::fmt::Display for ProjectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Identity(error) => write!(f, "documentation address: {error}"),
            Self::Annotation(error) => write!(f, "documentation declaration: {error}"),
        }
    }
}

impl std::error::Error for ProjectionError {}

impl From<scoop_wire::HashError> for ProjectionError {
    fn from(error: scoop_wire::HashError) -> Self {
        Self::Identity(error)
    }
}

impl From<hir::AnnotationProductionError> for ProjectionError {
    fn from(error: hir::AnnotationProductionError) -> Self {
        Self::Annotation(error)
    }
}

struct Projection<'a> {
    export: &'a hir::ExportHir,
    declarations: HashMap<Target, &'a hir::DocumentationDeclaration>,
    controls: HashMap<Source, annotations::Control>,
}

/// The caller supplies the already checked manifest coordinate and complete
/// HIR view. No filesystem reads or compiler queries occur in this stage.
pub fn project_cone(
    export: &hir::ExportHir,
    documentation: &hir::ConeDocumentation,
    coordinate: &ConeCoordinate,
    kind: RequestedConeKind,
) -> Result<ConeInput> {
    let mut projection = Projection {
        export,
        declarations: documentation
            .declarations
            .iter()
            .map(|item| (item.target, item))
            .collect(),
        controls: HashMap::new(),
    };
    let mut warnings = documentation
        .warnings
        .iter()
        .map(|warning| projection.warning(warning))
        .collect::<Result<Vec<_>>>()?;
    projection.collect_controls(documentation, &mut warnings)?;
    let items = documentation
        .declarations
        .iter()
        .map(|item| projection.item(item))
        .collect::<Result<Vec<_>>>()?;
    let sources = export
        .source_files
        .iter()
        .filter(|source| source.identity.cone() == export.cone)
        .map(|source| {
            Ok(SourceInput {
                key: SourceKey::new(&source.identity)?,
                path: source.identity.logical_path().as_str().into(),
                display_name: source.name.clone(),
                text: source.source.clone(),
            })
        })
        .collect::<Result<_>>()?;
    Ok(ConeInput {
        key: ConeKey::new(export.cone)?,
        name: coordinate.to_string(),
        executable: kind == RequestedConeKind::Executable,
        sources,
        items,
        bindings: projection.bindings(&documentation.hidden_dependency_bindings)?,
        warnings,
    })
}

impl Projection<'_> {
    fn item(&self, item: &hir::DocumentationDeclaration) -> Result<ApiItem> {
        let control = match item.target {
            Target::Declaration(target) => self.controls.get(&target),
            _ => None,
        };
        let alternate_keys = match item.target {
            Target::Declaration(Source::Nominal(hir::NominalOwner::Object(id))) => {
                vec![SymbolKey::object_value(
                    self.export.object_value_identities[self.export.objects[id].singleton_value]
                        .id(),
                )?]
            }
            _ => Vec::new(),
        };
        Ok(ApiItem {
            key: self.target(item.target)?,
            alternate_keys,
            parent: item.parent.map(|parent| self.target(parent)).transpose()?,
            package: item.package.clone(),
            kind: kind(item.kind),
            name: item.name.clone(),
            visibility: visibility::label(item.visibility),
            public: self.is_public(item.target),
            hidden: control.is_some_and(|control| control.hidden),
            aliases: control
                .map(|control| control.aliases.clone())
                .unwrap_or_default(),
            synthesized: item.synthesized,
            signature: self.signature(item)?,
            receiver: self.receiver(item)?,
            equality_requirements: self.equality_requirements(item)?,
            location: self.location(item.origin.file as usize, item.origin.span)?,
            comment: item
                .comment
                .as_ref()
                .map(|comment| self.comment(comment))
                .transpose()?,
            property: self.property(item.target)?,
            relations: self.relations(item)?,
        })
    }

    fn location(&self, file: usize, span: scoop_ast::Span) -> Result<SourceLocation> {
        let source = &self.export.source_files[file];
        Ok(SourceLocation {
            source: SourceKey::new(&source.identity)?,
            range: span.into(),
            line: source.source.as_bytes()[..span.start as usize]
                .iter()
                .filter(|&&byte| byte == b'\n')
                .count()
                + 1,
        })
    }
}

fn kind(kind: hir::DocumentationKind) -> ApiKind {
    use hir::DocumentationKind as K;
    match kind {
        K::Class => ApiKind::Class,
        K::Struct => ApiKind::Struct,
        K::Enum => ApiKind::Enum,
        K::Interface => ApiKind::Interface,
        K::Object => ApiKind::Object,
        K::Annotation => ApiKind::Annotation,
        K::TypeAlias => ApiKind::TypeAlias,
        K::Function => ApiKind::Function,
        K::Constructor => ApiKind::Constructor,
        K::Property => ApiKind::Property,
        K::Field => ApiKind::Field,
        K::Variant => ApiKind::Variant,
        K::Parameter => ApiKind::Parameter,
        K::TypeParameter => ApiKind::TypeParameter,
        K::ContextParameter => ApiKind::ContextParameter,
        K::Getter => ApiKind::Getter,
        K::Setter => ApiKind::Setter,
    }
}
