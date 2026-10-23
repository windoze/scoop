//! Render local MIR observations using existing identities and declarations.

use mir::optimization_report::{OptimizationDecision, OptimizationReport};
use scoop_hir::CanonicalHirFoundation;
use scoop_mir as mir;
use serde_json::{Value, json};

mod decisions;
mod identities;

pub(super) fn render(
    module: &mir::Module,
    selected: &mir::SelectedExternalMirSet,
    foundations: &[&CanonicalHirFoundation],
    report: &OptimizationReport,
) -> String {
    let functions = module.functions.iter().map(|(id, function)| {
        let source = module.meta.source_callable_materializations.get(id);
        let generated = module.meta.generated_callables.get(id);
        let template = source.map(|source| source.materialization().template()).or_else(||
            generated.map(|generated| scoop_identity::CallableTemplateOwner::Generated(generated.identity_record().id())));
        json!({
            "id": id.into_raw().into_u32(), "name": function.name,
            "parameters": function.params.iter().map(|p| type_info(module, &p.ty)).collect::<Vec<_>>(),
            "result": type_info(module, &function.return_ty),
            "implementation": module.meta.callable_signature_subject(id).map(identities::implementation),
            "materialization": source.map(|source| identities::materialization(source.materialization())),
            "declaration": template.and_then(|template| identities::origin(foundations, template)),
        })
    }).collect::<Vec<_>>();
    let externals = module.meta.external_callables.iter().map(|(id, callable)| {
        let reference = callable.reference();
        let selection = selected.resolve_callable(reference).expect("an external use has its selected signature");
        let owner = reference.implementation().callable_owner();
        let template = identities::owner_template(owner);
        json!({
            "id": id.into_raw().into_u32(), "provider": reference.provider().to_string(),
            "name": foundations.iter().find_map(|foundation| foundation.callable_names(template)).map(|names| {
                if names.type_name.is_empty() { names.function }
                else { format!("{}.{}", names.type_name, names.function) }
            }),
            "implementation": identities::implementation(mir::CallableSignatureSubject::Strong(owner)),
            "signature": identities::signature(selection.signature()),
            "declaration": identities::origin(foundations, template),
        })
    }).collect::<Vec<_>>();
    let native = module.extern_functions.iter().map(|(id, callable)| json!({
        "id": id.into_raw().into_u32(), "name": callable.source_name,
        "symbol": callable.native_symbol, "contract": callable.source_contract.id().to_string(),
    })).collect::<Vec<_>>();
    let events = report.events.iter().map(|event| {
        let site = &event.site;
        let mut value = match &event.decision {
            OptimizationDecision::Inline { target, outcome, estimate } => json!({
                "pass": "inline", "outcome": decisions::inline(*outcome),
                "target": target_info(module, target),
                "estimate": estimate.as_ref().map(decisions::estimate),
            }),
            OptimizationDecision::Allocation { kind, ty, outcome } => json!({
                "pass": "allocation-elision", "kind": decisions::allocation_kind(*kind),
                "type": type_info(module, ty), "outcome": decisions::allocation(*outcome),
            }),
        };
        let fields = value.as_object_mut().unwrap();
        fields.insert("round".into(), json!(event.round));
        fields.insert("site".into(), json!({
            "function": site.function.into_raw().into_u32(),
            "block": site.block.into_raw().into_u32(), "statement": site.statement,
            "span": identities::span(site.span),
            "inline_path": site.inline_path.iter().map(|c| callee_ref(module, *c)).collect::<Vec<_>>(),
        }));
        value
    }).collect::<Vec<_>>();
    let value = json!({
        "schema": 1, "cone": module.cone.to_string(),
        "passes": {"inline": report.inlining, "allocation-elision": report.allocation_elision},
        "rounds": report.rounds, "converged": report.converged,
        "functions": functions, "external_callables": externals,
        "native_callables": native, "events": events,
    });
    let mut text = serde_json::to_string_pretty(&value).expect("report values are JSON data");
    text.push('\n');
    text
}

fn type_info(module: &mir::Module, ty: &mir::Type) -> Value {
    json!({"name": mir::type_name(module, ty),
        "exact": module.meta.source_exact_types.get(ty).map(|record| record.identity_record().id().to_string())})
}

fn callee_ref(module: &mir::Module, callee: mir::Callee) -> Value {
    match callee {
        mir::Callee::User(id) => json!({"kind": "function", "id": id.into_raw().into_u32()}),
        mir::Callee::Monomorphized(id) => callee_ref(
            module,
            mir::Callee::User(module.meta.instances[id].function),
        ),
        mir::Callee::External(id) => json!({"kind": "external", "id": id.into_raw().into_u32()}),
        mir::Callee::Extern(id) => json!({"kind": "native", "id": id.into_raw().into_u32()}),
        mir::Callee::Runtime(function) => json!({"kind": "runtime", "symbol": function.symbol()}),
    }
}

fn target_info(module: &mir::Module, target: &mir::CallTarget<()>) -> Value {
    match *target {
        mir::CallTarget::Direct(callee) => {
            json!({"kind": "direct", "callee": callee_ref(module, callee)})
        }
        mir::CallTarget::Virtual { callee, slot, .. } => json!({
            "kind": "virtual", "callee": callee_ref(module, callee.into()), "slot": slot,
        }),
        mir::CallTarget::Interface {
            callee,
            interface,
            slot,
            ..
        } => json!({
            "kind": "interface", "callee": callee_ref(module, callee.into()), "slot": slot,
            "interface": type_info(module, &mir::Type::Interface(interface)),
        }),
        mir::CallTarget::Closure { function_type, .. } => json!({
            "kind": "closure", "type": type_info(module, &mir::Type::Function(function_type)),
        }),
        mir::CallTarget::FunctionBridge { function_type, .. } => json!({
            "kind": "function-bridge", "type": type_info(module, &mir::Type::Function(function_type)),
        }),
    }
}
