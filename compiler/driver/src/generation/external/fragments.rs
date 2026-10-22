use super::*;
use scoop_generator_protocol::GeneratedBody;
use scoop_identity::{NormalizedSourcePath, SourceIdentity};
use std::collections::HashSet;

pub(super) struct Fragment {
    method: GeneratorMethod,
    source: hir::GeneratedMethodSource,
}

pub(super) fn collect(
    cone: ConeIdentity,
    batch: &GeneratorBatch,
    bodies: Vec<GeneratedBody>,
    sources: &mut Vec<hir::GeneratedMethodSource>,
    errors: &mut Vec<Diagnostic>,
) -> Vec<Fragment> {
    let methods = batch
        .tasks
        .iter()
        .flat_map(|task| task.methods.iter())
        .map(|method| (method.method, method))
        .collect::<HashMap<_, _>>();
    let fallback = &batch
        .tasks
        .first()
        .expect("planned batch")
        .methods
        .first()
        .origin;
    let mut seen = HashSet::new();
    let mut fragments = Vec::new();
    for body in bodies {
        let Some(method) = methods.get(&body.method) else {
            errors.push(diagnostic(
                fallback,
                "generator returned a method outside its pending batch",
            ));
            continue;
        };
        if !seen.insert(body.method) {
            errors.push(diagnostic(
                &method.origin,
                "generator returned the same method more than once",
            ));
            continue;
        }
        let path = match body.method {
            MethodId::Function(id) => format!("generated/function/{id}.scoop"),
            MethodId::GenericFunction(id) => format!("generated/generic-function/{id}.scoop"),
        };
        let source = hir::GeneratedMethodSource {
            identity: SourceIdentity::new(
                cone,
                NormalizedSourcePath::new(&path).expect("method-derived source path is normalized"),
            ),
            name: path,
            text: body.source.into(),
        };
        sources.push(source.clone());
        fragments.push(Fragment {
            method: (*method).clone(),
            source,
        });
    }
    fragments
}

pub(super) fn parse(
    fragments: Vec<Fragment>,
) -> Result<Vec<hir::GeneratedMethodBody>, Vec<Diagnostic>> {
    let mut bodies = Vec::new();
    let mut errors = Vec::new();
    for Fragment { method, source } in fragments {
        match scoop_parser::parse_function_body(source.text.as_str()) {
            Ok(body) => bodies.push(hir::GeneratedMethodBody {
                function: method.pending.function,
                body,
                source,
                origin: method.origin,
            }),
            Err(diagnostics) => errors.extend(diagnostics.into_iter().map(|mut error| {
                error.source = Some(Box::new(source.identity.clone()));
                for note in &mut error.notes {
                    note.source = Some(Box::new(source.identity.clone()));
                }
                error
                    .notes
                    .push(related(&method.origin, "implementation requested here"));
                error
            })),
        }
    }
    if errors.is_empty() {
        Ok(bodies)
    } else {
        Err(errors)
    }
}
