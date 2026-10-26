use scoop_identity::{
    CallableApplicationKey, CallableTemplateOrigin, CallableTemplateOwner, GeneratedCallableKey,
    OdrMemberDiscriminator, OdrMemberKey, PersistentCallableApplicationId, ValidatedIdentityGraph,
};
use scoop_mir::CallableSignatureSubject;

use super::identities;

pub(super) fn template(
    subject: CallableSignatureSubject,
    graph: &ValidatedIdentityGraph,
) -> (
    Option<CallableTemplateOwner>,
    Option<PersistentCallableApplicationId>,
) {
    match subject {
        CallableSignatureSubject::Strong(owner) => (Some(identities::owner_template(owner)), None),
        CallableSignatureSubject::Odr(member) => {
            let Ok(key) = graph.canonical_key::<_, OdrMemberKey>(member.member()) else {
                return (None, None);
            };
            match key.discriminator() {
                OdrMemberDiscriminator::CallableApplication(id) => {
                    let template = graph
                        .canonical_key::<_, CallableApplicationKey>(*id)
                        .ok()
                        .map(|key| origin_template(key.origin()));
                    (template, Some(*id))
                }
                OdrMemberDiscriminator::GeneratedCallable(id) => {
                    (Some(CallableTemplateOwner::Generated(*id)), None)
                }
                _ => (None, None),
            }
        }
    }
}

fn origin_template(origin: CallableTemplateOrigin) -> CallableTemplateOwner {
    match origin {
        CallableTemplateOrigin::Function(id) => CallableTemplateOwner::Function(id),
        CallableTemplateOrigin::GenericFunction(id) => CallableTemplateOwner::GenericFunction(id),
        CallableTemplateOrigin::Constructor(id) => CallableTemplateOwner::Constructor(id),
        CallableTemplateOrigin::Accessor(id) => CallableTemplateOwner::Accessor(id),
        CallableTemplateOrigin::VariantConstructor(id) => {
            CallableTemplateOwner::VariantConstructor(id)
        }
    }
}

pub(super) fn generated_role(
    template: CallableTemplateOwner,
    graph: &ValidatedIdentityGraph,
) -> Option<&'static str> {
    let CallableTemplateOwner::Generated(id) = template else {
        return None;
    };
    let key = graph.canonical_key::<_, GeneratedCallableKey>(id).ok()?;
    Some(match key.as_ref() {
        GeneratedCallableKey::AnnotationDescription { .. } => "annotation-description",
        GeneratedCallableKey::TestEntry { .. } => "test-entry",
        GeneratedCallableKey::Lexical { .. } => "lexical-callable",
        GeneratedCallableKey::Initialization { .. } => "initialization",
        GeneratedCallableKey::DerivedEquality { .. } => "derived-equality",
        GeneratedCallableKey::FunctionAdapter { .. } => "function-adapter",
        GeneratedCallableKey::DynamicFunctionAdapter { .. } => "dynamic-function-adapter",
        GeneratedCallableKey::CallableReferenceInvoke { .. } => "callable-reference-invoke",
        GeneratedCallableKey::StaticNoGcCallbackStorageBridge { .. } => "callback-storage-bridge",
        GeneratedCallableKey::ForeignCallbackManagedAdapter { .. } => "foreign-callback-adapter",
        GeneratedCallableKey::CoroutineDriver { .. } => "coroutine-driver",
        GeneratedCallableKey::CoroutineStart { .. } => "coroutine-start",
        GeneratedCallableKey::CoroutineAdapter { .. } => "coroutine-adapter",
        GeneratedCallableKey::FunctionBridge { .. } => "function-bridge",
        GeneratedCallableKey::DispatchAdjust { .. } => "dispatch-adjust",
        GeneratedCallableKey::BoxingAdjust { .. } => "boxing-adjust",
        GeneratedCallableKey::ZeroArgumentConstructorAdapter { .. } => "constructor-adapter",
    })
}
