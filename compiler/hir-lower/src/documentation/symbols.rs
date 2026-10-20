use super::binding::DocumentationIndex;
use super::*;
use crate::imports::lookup::{calls::NamedCallTarget, values::ValueTarget};
use crate::namespace::TopLevelTypeTarget;
use hir::{AnnotationTargetV1 as Imported, DocumentationSymbol as Symbol};

mod members;
mod targets;

impl Lowerer {
    pub(super) fn documentation_symbols(
        &mut self,
        declaration: Target,
        path: &ast::QualifiedNameSyntax,
        index: &DocumentationIndex,
        world: Option<&hir::ImportedSemanticWorld<'_>>,
    ) -> Result<Vec<Symbol>, &'static str> {
        let segments = path.segments().cloned().collect::<Vec<_>>();
        let (mut symbols, consumed) =
            if let Some(package) = self.qualified_package_prefix(&segments) {
                let consumed = package.consumed() + 1;
                if consumed > segments.len() {
                    return Err("names a package, not a declaration");
                }
                let head = ast::QualifiedNameSyntax {
                    first: path.first.clone(),
                    rest: path.rest[..consumed - 1].to_vec(),
                    span: ast::Span::new(path.span.start, segments[consumed - 1].span.end),
                };
                let targets = self
                    .imports
                    .documentation_qualified_targets(self, world, &head)
                    .map_err(|_| "cannot be resolved")?;
                (self.documentation_named_targets(targets, world), consumed)
            } else {
                (
                    self.documentation_name(declaration, &path.first.text, index, world),
                    1,
                )
            };
        for name in &segments[consumed..] {
            symbols = canonical_symbols(symbols)?;
            if symbols.len() != 1 {
                return Err("has an ambiguous owner");
            }
            symbols = self.documentation_members(symbols[0], &name.text, world);
        }
        canonical_symbols(symbols)
    }

    fn documentation_name(
        &mut self,
        declaration: Target,
        name: &str,
        index: &DocumentationIndex,
        world: Option<&hir::ImportedSemanticWorld<'_>>,
    ) -> Vec<Symbol> {
        if let Some(parameter) = index.lexical_parameter(declaration, name) {
            return vec![Symbol::Current(parameter)];
        }
        if let Some(alias) = self.lexical_type_alias(name) {
            return self.documentation_named_targets(
                vec![NamedCallTarget::Type(TopLevelTypeTarget::Alias(alias))],
                world,
            );
        }
        let mut owner = self.current_owner;
        while let Some(current) = owner {
            let symbols = self.documentation_members(
                Symbol::Current(Source::Nominal(current.as_nominal_owner()).into()),
                name,
                world,
            );
            if !symbols.is_empty() {
                return symbols;
            }
            owner = self.nominal_parent(current);
        }
        for layer in self.named_call_layers(name) {
            if !layer.candidates.is_empty() {
                return self.documentation_named_targets(
                    layer
                        .candidates
                        .into_iter()
                        .map(|binding| binding.target)
                        .collect(),
                    world,
                );
            }
        }
        Vec::new()
    }
}

fn canonical_symbols(symbols: Vec<Symbol>) -> Result<Vec<Symbol>, &'static str> {
    let mut unique = Vec::new();
    for symbol in symbols {
        if !unique.contains(&symbol) {
            unique.push(symbol);
        }
    }
    if unique.is_empty() {
        return Err("cannot be resolved");
    }
    if unique.len() > 1
        && !unique.iter().all(|symbol| {
            matches!(
                symbol,
                Symbol::Current(Target::Declaration(Source::Callable(Callable::Function(_))))
                    | Symbol::Current(Target::DerivedFunction(_))
                    | Symbol::Imported(Imported::Callable(hir::AnnotationCallableV1::Declaration(
                        scoop_identity::CallableTemplateOrigin::Function(_)
                            | scoop_identity::CallableTemplateOrigin::GenericFunction(_)
                    )))
            )
        })
    {
        return Err("is ambiguous");
    }
    Ok(unique)
}
