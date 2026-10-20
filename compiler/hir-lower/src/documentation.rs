//! Definition-site documentation uses the ordinary typed declaration graph.

use std::collections::{BTreeMap, HashSet};

use scoop_ast as ast;
use scoop_hir as hir;

use crate::{Lowerer, Owner};
use hir::{
    DocumentationKind as Kind, DocumentationSignaturePart as Part, DocumentationTarget as Target,
    SourceAnnotationCallable as Callable, SourceAnnotationTarget as Source,
};

mod binding;
mod constructors;
mod functions;
mod nominals;
mod properties;
mod signatures;
mod symbols;
mod synthesized;

#[derive(Clone)]
pub(crate) struct DocumentationContext {
    comments: BTreeMap<(usize, u32, u32), ast::DocDocument>,
    declarations: Vec<CapturedDeclaration>,
    targets: HashSet<Target>,
    warnings: Vec<ast::Diagnostic>,
}

#[derive(Clone)]
struct CapturedDeclaration {
    declaration: hir::DocumentationDeclaration,
    comment: Option<ast::DocDocument>,
    scope: DocumentationScope,
}

#[derive(Clone)]
struct DocumentationScope {
    file: usize,
    owner: Option<Owner>,
    parameters: Vec<hir::TypeParamDecl>,
}

impl DocumentationContext {
    pub(crate) fn new(sources: &[ast::SourceDocumentation]) -> Self {
        let mut result = Self {
            comments: BTreeMap::new(),
            declarations: Vec::new(),
            targets: HashSet::new(),
            warnings: Vec::new(),
        };
        for (file, source) in sources.iter().enumerate() {
            for comment in &source.documents {
                let span = comment.declaration;
                result
                    .comments
                    .insert((file, span.start, span.end), comment.clone());
            }
            result
                .warnings
                .extend(source.warnings.iter().cloned().map(|mut warning| {
                    warning.file = file;
                    warning
                }));
        }
        result
    }
}

impl Lowerer {
    #[allow(clippy::too_many_arguments)]
    fn document_declaration(
        &mut self,
        target: Target,
        parent: Option<Target>,
        kind: Kind,
        name: &str,
        span: ast::Span,
        visibility: hir::DeclaredVisibility,
        signature: Vec<Part>,
        attach: bool,
    ) {
        if self.documentation.is_none() {
            return;
        }
        // Locally declared types and callable bodies have no site pages.
        if self
            .current_owner
            .is_some_and(|owner| self.local_type_root(owner).is_some())
        {
            return;
        }
        let origin = self.definition_origin(span);
        let package = match self
            .top_level_namespaces
            .source_namespace(self.current_file)
        {
            crate::namespace::TopLevelLookupLayer::CorePrelude => Vec::new(),
            crate::namespace::TopLevelLookupLayer::CurrentPackage(package) => self
                .top_level_namespaces
                .package_segments(package)
                .into_iter()
                .map(str::to_owned)
                .collect(),
        };
        let scope = DocumentationScope {
            file: self.current_file,
            owner: self.current_owner,
            parameters: self.type_params_in_scope.clone(),
        };
        let context = self
            .documentation
            .as_mut()
            .expect("documentation is requested");
        if !context.targets.insert(target) {
            return;
        }
        let comment = attach
            .then(|| {
                context
                    .comments
                    .remove(&(self.current_file, span.start, span.end))
            })
            .flatten();
        context.declarations.push(CapturedDeclaration {
            declaration: hir::DocumentationDeclaration {
                target,
                parent,
                kind,
                name: name.into(),
                package,
                origin,
                visibility,
                synthesized: false,
                signature,
                type_parameters: scope.parameters.clone(),
                comment: None,
            },
            comment,
            scope,
        });
    }

    fn document_type_parameters(
        &mut self,
        owner: Source,
        source: &[ast::TypeParamDecl],
        parameters: &[hir::TypeParamDecl],
    ) {
        for (source, parameter) in source.iter().zip(parameters) {
            self.document_declaration(
                Target::TypeParameter {
                    owner,
                    parameter: parameter.id,
                },
                Some(owner.into()),
                Kind::TypeParameter,
                &source.name.text,
                source.span,
                hir::DeclaredVisibility::Public,
                vec![Part::Source(source.span)],
                true,
            );
        }
    }

    fn document_parameter(
        &mut self,
        callable: Callable,
        index: usize,
        name: &str,
        span: ast::Span,
        attach: bool,
    ) {
        self.document_declaration(
            Source::Parameter {
                callable,
                index: index as u32,
            }
            .into(),
            Some(Source::Callable(callable).into()),
            Kind::Parameter,
            name,
            span,
            hir::DeclaredVisibility::Public,
            vec![Part::Source(span)],
            attach,
        );
    }

    fn documentation_owner(owner: Option<Owner>) -> Option<Target> {
        owner.map(|owner| Source::Nominal(owner.as_nominal_owner()).into())
    }
}
