//! Function-level attribution from the identities already loaded by compilation.

use scoop_hir::CanonicalHirFoundation;
use scoop_identity::{
    CallableBodyKeyKind, CallableTemplateOwner, MangledSymbol, PersistentSymbolKey,
    ValidatedIdentityGraph,
};
use scoop_lir::ConeLirFoundation;
use scoop_mir::{CallableSignatureSubject, CanonicalMirFoundation};
use scoop_slib::PhysicalImportsReplayedCrossConeLayoutSections;
use serde_json::{Value, json};

use super::identities;
mod sources;

pub(super) fn render(
    mir: &CanonicalMirFoundation,
    lir: &ConeLirFoundation,
    hir: &CanonicalHirFoundation,
    graph: &ValidatedIdentityGraph,
    dependencies: &[&PhysicalImportsReplayedCrossConeLayoutSections],
) -> String {
    let foundations = std::iter::once(hir)
        .chain(
            dependencies
                .iter()
                .map(|dependency| dependency.hir_foundation()),
        )
        .collect::<Vec<_>>();
    let mut symbols = cone(mir, lir, graph, &foundations);
    for dependency in dependencies {
        symbols.extend(cone(
            dependency.mir_foundation(),
            dependency.lir_foundation(),
            dependency.identity_graph(),
            &foundations,
        ));
    }
    symbols.sort_by(|left, right| {
        (left["symbol"].as_str(), left["producer"].as_str())
            .cmp(&(right["symbol"].as_str(), right["producer"].as_str()))
    });
    let mut output = serde_json::to_string_pretty(&json!({
        "schema": 1, "cone": lir.producer().to_string(), "symbols": symbols,
    }))
    .expect("symbol observations are JSON data");
    output.push('\n');
    output
}

fn cone(
    mir: &CanonicalMirFoundation,
    lir: &ConeLirFoundation,
    graph: &ValidatedIdentityGraph,
    foundations: &[&CanonicalHirFoundation],
) -> Vec<Value> {
    lir.callable_bodies()
        .iter()
        .map(|body| {
            let (role, subject) = match body.key().kind() {
                CallableBodyKeyKind::Strong(owner) => {
                    ("callable", Some(CallableSignatureSubject::Strong(owner.callable_owner())))
                }
                CallableBodyKeyKind::Odr(member) => {
                    ("callable", Some(CallableSignatureSubject::Odr(member)))
                }
                CallableBodyKeyKind::RootGateway { .. } => ("root-gateway", None),
                CallableBodyKeyKind::InitializationStartupGateway(_) => {
                    ("initialization-gateway", None)
                }
                CallableBodyKeyKind::ReleaseHook { .. } => ("release-hook", None),
            };
            let (template, application) = subject
                .map(|subject| sources::template(subject, graph))
                .unwrap_or_default();
            let role = template
                .and_then(|template| sources::generated_role(template, graph))
                .unwrap_or(role);
            let name = template.and_then(|template| callable_name(foundations, template));
            json!({
                "symbol": MangledSymbol::from_key(&PersistentSymbolKey::CallableBody(body.id())).as_str(),
                "body": body.id().to_string(), "producer": lir.producer().to_string(),
                "role": role, "name": name,
                "implementation": subject.map(identities::implementation),
                "template": template.map(identities::template),
                "application": application.map(|id| id.to_string()),
                "signature": subject.and_then(|subject| mir.callable_signature(subject))
                    .map(|record| identities::signature(record.signature())),
                "declaration": template.and_then(|template| identities::origin(foundations, template)),
            })
        })
        .collect()
}

fn callable_name(
    foundations: &[&CanonicalHirFoundation],
    template: CallableTemplateOwner,
) -> Option<String> {
    foundations
        .iter()
        .find_map(|foundation| foundation.callable_names(template))
        .map(|names| {
            if names.type_name.is_empty() {
                names.function
            } else {
                format!("{}.{}", names.type_name, names.function)
            }
        })
}
