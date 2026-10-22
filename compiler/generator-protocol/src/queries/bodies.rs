use super::*;
use hir::{AnnotationCallableV1, AnnotationTargetV1, SemanticBodyAvailability as Availability};
use hir::{SemanticCallable as Callable, SemanticConstructor as Constructor};
use hir::{SourceAnnotationCallable as Source, SourceAnnotationTarget};
use scoop_identity::CallableTemplateOrigin as Declaration;
use std::sync::Arc;

pub(super) fn describe(
    queries: &mut dyn hir::SemanticQueries,
    reference: CallableDeclaration,
) -> Result<Value> {
    let Some(declaration) = queries
        .resolve_declaration(hir::DecodedAnnotationTargetV1::Callable(reference.into()))
        .map_err(one)?
    else {
        return Ok(Value::Null);
    };
    match body_reference(declaration.annotations) {
        BodyReference::Callable(callable) => {
            let body = queries
                .projected_body(callable)
                .map_err(one)?
                .expect("a resolved callable retains its body availability");
            body_result::<hir::ExportGenericCallableBodyV1>(body, "callable")
        }
        BodyReference::Constructor(constructor) => {
            let body = queries
                .constructor_body(constructor)
                .map_err(one)?
                .expect("a resolved constructor retains its body availability");
            body_result::<hir::SemanticConstructorBody>(body, "constructor")
        }
        BodyReference::NoBody => Ok(json!({"availability": "noBody"})),
    }
}

fn body_result<T>(body: Availability<Arc<T>>, kind: &str) -> Result<Value>
where
    for<'a> crate::BodyJson<'a, T>: Serialize,
{
    Ok(match body {
        Availability::Available(body) => json!({
            "availability": "available", "kind": kind,
            "body": encode(crate::BodyJson::new(body.as_ref()))?,
        }),
        Availability::PendingGeneration => json!({"availability": "pendingGeneration"}),
        Availability::NotPublished => json!({"availability": "notPublished"}),
        Availability::NoBody => json!({"availability": "noBody"}),
    })
}

enum BodyReference {
    Callable(Callable),
    Constructor(Constructor),
    NoBody,
}

fn body_reference(target: hir::SemanticAnnotationTarget) -> BodyReference {
    use hir::DefaultCallableDeclarationV1 as Imported;
    match target {
        hir::SemanticAnnotationTarget::Current(SourceAnnotationTarget::Callable(source)) => {
            match source {
                Source::Function(id) => BodyReference::Callable(Callable::Current(id)),
                Source::Getter(id) => BodyReference::Callable(Callable::CurrentGetter(id)),
                Source::Setter(id) => BodyReference::Callable(Callable::CurrentSetter(id)),
                Source::StructConstructor(id) => {
                    BodyReference::Constructor(Constructor::CurrentStruct(id))
                }
                Source::ClassConstructor(id) => {
                    BodyReference::Constructor(Constructor::CurrentClass(id))
                }
                Source::VariantConstructor(_) => BodyReference::NoBody,
            }
        }
        hir::SemanticAnnotationTarget::Imported(AnnotationTargetV1::Callable(callable)) => {
            let callable = match callable {
                AnnotationCallableV1::Declaration(declaration) => match declaration {
                    Declaration::Function(id) => Imported::Function(id),
                    Declaration::GenericFunction(id) => Imported::GenericFunction(id),
                    Declaration::Accessor(id) => Imported::PropertyAccessor(id),
                    Declaration::Constructor(id) => {
                        return BodyReference::Constructor(Constructor::Imported(id));
                    }
                    Declaration::VariantConstructor(_) => return BodyReference::NoBody,
                },
                AnnotationCallableV1::Local(id) => Imported::Generated(id),
            };
            BodyReference::Callable(Callable::Imported(callable))
        }
        _ => unreachable!("a resolved callable retains its callable target"),
    }
}
