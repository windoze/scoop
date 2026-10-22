//! Preserve the selected callable/property domain when requesting its comment.

use scoop_hir::{
    self as hir, DocumentationSymbol as Symbol, SourceAnnotationCallable as SourceCallable,
    SourceAnnotationTarget as Source,
};

pub(in crate::queries::hover) fn callable(callable: hir::SemanticCallable) -> Symbol {
    match callable {
        hir::SemanticCallable::Current(function) => {
            Symbol::Current(Source::Callable(SourceCallable::Function(function)).into())
        }
        hir::SemanticCallable::CurrentGetter(getter) => {
            Symbol::Current(Source::Callable(SourceCallable::Getter(getter)).into())
        }
        hir::SemanticCallable::CurrentSetter(setter) => {
            Symbol::Current(Source::Callable(SourceCallable::Setter(setter)).into())
        }
        hir::SemanticCallable::Imported(callable) => {
            use hir::{AnnotationCallableV1 as Callable, DefaultCallableDeclarationV1 as D};
            use scoop_identity::CallableTemplateOrigin as Origin;
            let callable = match callable {
                D::Function(id) => Callable::Declaration(Origin::Function(id)),
                D::GenericFunction(id) => Callable::Declaration(Origin::GenericFunction(id)),
                D::PropertyAccessor(id) => Callable::Declaration(Origin::Accessor(id)),
                D::Generated(id) => Callable::Local(id),
            };
            Symbol::Imported(hir::AnnotationTargetV1::Callable(callable))
        }
    }
}

pub(in crate::queries::hover) fn parameter(function: hir::FunctionId, index: usize) -> Symbol {
    Symbol::Current(
        Source::Parameter {
            callable: SourceCallable::Function(function),
            index: index as u32,
        }
        .into(),
    )
}

pub(in crate::queries::hover) fn property(property: hir::SemanticProperty) -> Symbol {
    match property {
        hir::SemanticProperty::Current(property) => {
            Symbol::Current(Source::Property(property).into())
        }
        hir::SemanticProperty::Imported(property) => Symbol::Imported(match property {
            hir::PropertyDeclarationId::Property(id) => hir::AnnotationTargetV1::Property(id),
            hir::PropertyDeclarationId::ExtensionProperty(id) => {
                hir::AnnotationTargetV1::ExtensionProperty(id)
            }
        }),
    }
}
