use super::*;

impl Projection<'_> {
    pub(super) fn source_target(&self, target: Source) -> Result<SymbolKey> {
        Ok(SymbolKey::declaration(
            hir::AnnotationTargetV1::from_source(self.export, target)?,
        )?)
    }

    pub(super) fn target(&self, target: Target) -> Result<SymbolKey> {
        Ok(match target {
            Target::Declaration(source) => return self.source_target(source),
            Target::TypeParameter { owner, parameter } => {
                let declaration = self.declarations[&Target::Declaration(owner)];
                let position = declaration
                    .type_parameters
                    .iter()
                    .position(|candidate| candidate.id == parameter)
                    .expect("a documentation binder belongs to its actual owner");
                self.source_target(owner)?.child("type", position as u32)
            }
            Target::ContextParameter { owner, index } => {
                self.source_target(owner)?.child("context", index)
            }
            Target::AnnotationParameter { annotation, index } => self
                .source_target(Source::Annotation(annotation))?
                .child("parameter", index),
            Target::DerivedFunction(_) => {
                let parent = self.declarations[&target]
                    .parent
                    .expect("a derived equality has its actual nominal owner");
                let Target::Declaration(source) = parent else {
                    unreachable!("a derived equality owner is nominal")
                };
                let hir::AnnotationTargetV1::Nominal(owner) =
                    hir::AnnotationTargetV1::from_source(self.export, source)?
                else {
                    unreachable!("a derived equality owner is nominal")
                };
                SymbolKey::derived_equality(owner)?
            }
        })
    }

    pub(super) fn symbol(&self, symbol: hir::DocumentationSymbol) -> Result<SymbolKey> {
        match symbol {
            hir::DocumentationSymbol::Current(target) => self.target(target),
            hir::DocumentationSymbol::Imported(target) => Ok(SymbolKey::declaration(target)?),
            hir::DocumentationSymbol::ImportedDerivedEquality(owner) => {
                Ok(SymbolKey::derived_equality(owner)?)
            }
        }
    }
}
