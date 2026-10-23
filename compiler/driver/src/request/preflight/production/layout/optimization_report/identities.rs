use scoop_identity::{
    CallableMaterialization, CallableMaterializationContext, CallableOwner, CallableTemplateOwner,
    DefinitionOriginSubject, ExactCallableSignature, SourceSpan,
};
use serde_json::{Value, json};

pub(super) fn materialization(value: CallableMaterialization) -> Value {
    json!({"template": template(value.template()), "context": match value.context() {
        CallableMaterializationContext::NoSubstitution => json!({"kind": "none"}),
        CallableMaterializationContext::Application(id) => json!({"kind": "application", "id": id.to_string()}),
        CallableMaterializationContext::InitializationApplication(id) => json!({"kind": "initialization", "id": id.to_string()}),
    }})
}

fn template(value: CallableTemplateOwner) -> Value {
    let (kind, id) = match value {
        CallableTemplateOwner::Function(id) => ("function", id.to_string()),
        CallableTemplateOwner::GenericFunction(id) => ("generic-function", id.to_string()),
        CallableTemplateOwner::Constructor(id) => ("constructor", id.to_string()),
        CallableTemplateOwner::Accessor(id) => ("accessor", id.to_string()),
        CallableTemplateOwner::Generated(id) => ("generated", id.to_string()),
        CallableTemplateOwner::VariantConstructor(id) => ("variant-constructor", id.to_string()),
        CallableTemplateOwner::ReleaseHook(id) => ("release-hook", id.to_string()),
    };
    json!({"kind": kind, "id": id})
}

pub(super) fn owner_template(owner: CallableOwner) -> CallableTemplateOwner {
    match owner {
        CallableOwner::Function(id) => CallableTemplateOwner::Function(id),
        CallableOwner::GenericTemplate(id) => CallableTemplateOwner::GenericFunction(id),
        CallableOwner::Constructor(id) => CallableTemplateOwner::Constructor(id),
        CallableOwner::Accessor(id) => CallableTemplateOwner::Accessor(id),
        CallableOwner::Generated(id) => CallableTemplateOwner::Generated(id),
        CallableOwner::Application(_) => {
            unreachable!("strong implementations are not callable applications")
        }
    }
}

pub(super) fn implementation(subject: scoop_mir::CallableSignatureSubject) -> Value {
    match subject {
        scoop_mir::CallableSignatureSubject::Strong(CallableOwner::Application(id)) => {
            json!({"kind": "application", "id": id.to_string()})
        }
        scoop_mir::CallableSignatureSubject::Strong(owner) => {
            json!({"kind": "strong", "owner": template(owner_template(owner))})
        }
        scoop_mir::CallableSignatureSubject::Odr(member) => {
            json!({"kind": "odr", "id": member.member().to_string()})
        }
    }
}

pub(super) fn signature(value: &ExactCallableSignature) -> Value {
    json!({"effect": format!("{:?}", value.effect()),
        "receiver": value.receiver().into_option().map(|id| id.to_string()),
        "parameters": value.parameters().iter().map(ToString::to_string).collect::<Vec<_>>(),
        "result": value.result().to_string()})
}

pub(super) fn origin(
    foundations: &[&scoop_hir::CanonicalHirFoundation],
    owner: CallableTemplateOwner,
) -> Option<Value> {
    let subject = match owner {
        CallableTemplateOwner::Function(id) => DefinitionOriginSubject::Function(id),
        CallableTemplateOwner::GenericFunction(id) => DefinitionOriginSubject::GenericFunction(id),
        CallableTemplateOwner::Constructor(id) => DefinitionOriginSubject::Constructor(id),
        CallableTemplateOwner::Accessor(id) => DefinitionOriginSubject::PropertyAccessor(id),
        CallableTemplateOwner::Generated(id) => DefinitionOriginSubject::GeneratedCallable(id),
        CallableTemplateOwner::VariantConstructor(id) => DefinitionOriginSubject::EnumVariant(id),
        CallableTemplateOwner::ReleaseHook(_) => return None,
    };
    let origin = foundations
        .iter()
        .find_map(|foundation| foundation.definition_origin(subject))?
        .origin();
    Some(
        json!({"source": {"cone": origin.source().cone().to_string(),
        "path": origin.source().logical_path().as_str()}, "span": span(origin.span())}),
    )
}

pub(super) fn span(value: SourceSpan) -> Value {
    json!({"start_byte": value.start_byte(), "end_byte": value.end_byte()})
}
