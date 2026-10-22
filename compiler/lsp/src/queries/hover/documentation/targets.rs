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
        hir::SemanticCallable::Imported(callable) => Symbol::Imported(
            hir::AnnotationTargetV1::Callable(imported_callable(callable)),
        ),
    }
}

pub(in crate::queries::hover) fn parameter(function: hir::FunctionId, index: usize) -> Symbol {
    current_parameter(SourceCallable::Function(function), index)
}

pub(in crate::queries::hover) fn applied_parameter(
    callee: hir::EditingCallee,
    index: usize,
) -> Symbol {
    use hir::{EditingCallee as C, SemanticConstructor as Constructor};
    let callable = match callee {
        C::Callable(callable) => callable,
        C::Constructor(Constructor::CurrentClass(constructor)) => {
            return current_parameter(SourceCallable::ClassConstructor(constructor), index);
        }
        C::Constructor(Constructor::CurrentStruct(constructor)) => {
            return current_parameter(SourceCallable::StructConstructor(constructor), index);
        }
        C::CurrentVariant(variant) => {
            return current_parameter(SourceCallable::VariantConstructor(variant), index);
        }
        C::Constructor(Constructor::Imported(constructor)) => {
            return imported_parameter(
                scoop_identity::CallableTemplateOrigin::Constructor(constructor),
                index,
            );
        }
        C::ImportedVariant(variant) => {
            return imported_parameter(
                scoop_identity::CallableTemplateOrigin::VariantConstructor(variant),
                index,
            );
        }
    };
    match callable {
        hir::SemanticCallable::Current(function) => {
            current_parameter(SourceCallable::Function(function), index)
        }
        hir::SemanticCallable::CurrentGetter(getter) => {
            current_parameter(SourceCallable::Getter(getter), index)
        }
        hir::SemanticCallable::CurrentSetter(setter) => {
            current_parameter(SourceCallable::Setter(setter), index)
        }
        hir::SemanticCallable::Imported(callable) => {
            Symbol::Imported(hir::AnnotationTargetV1::Parameter {
                callable: imported_callable(callable),
                index: index as u32,
            })
        }
    }
}

fn imported_parameter(callable: scoop_identity::CallableTemplateOrigin, index: usize) -> Symbol {
    Symbol::Imported(hir::AnnotationTargetV1::Parameter {
        callable: hir::AnnotationCallableV1::Declaration(callable),
        index: index as u32,
    })
}

fn current_parameter(callable: SourceCallable, index: usize) -> Symbol {
    Symbol::Current(
        Source::Parameter {
            callable,
            index: index as u32,
        }
        .into(),
    )
}

fn imported_callable(callable: hir::DefaultCallableDeclarationV1) -> hir::AnnotationCallableV1 {
    use hir::{AnnotationCallableV1 as Callable, DefaultCallableDeclarationV1 as D};
    use scoop_identity::CallableTemplateOrigin as Origin;
    match callable {
        D::Function(id) => Callable::Declaration(Origin::Function(id)),
        D::GenericFunction(id) => Callable::Declaration(Origin::GenericFunction(id)),
        D::PropertyAccessor(id) => Callable::Declaration(Origin::Accessor(id)),
        D::Generated(id) => Callable::Local(id),
    }
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
