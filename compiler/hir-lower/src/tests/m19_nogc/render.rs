use super::*;

fn selected(name: &str) -> bool {
    matches!(
        name,
        "Scalar"
            | "Empty"
            | "Chain"
            | "Defaulted"
            | "Native"
            | "Phantom"
            | "Wrapper"
            | "WrapperValue"
            | "Recursive"
    )
}

pub(super) fn contracts(output: &hir::Output, mir: &scoop_mir::Module) -> String {
    let mut rows = Vec::new();
    for (_, c) in output.export.struct_constructors.iter() {
        let owner = &output.export.structs[c.owner];
        if !selected(&owner.name) {
            continue;
        }
        let names = c
            .parameters
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>()
            .join(",");
        let required = c
            .no_gc_type_params
            .iter()
            .map(|id| {
                owner
                    .type_params
                    .iter()
                    .find(|p| p.id == *id)
                    .unwrap()
                    .name
                    .as_str()
            })
            .collect::<Vec<_>>();
        rows.push(format!(
            "source {}({names}): {:?}; requires {required:?}\n",
            owner.name,
            c.source_gc_effect()
        ));
    }
    for (_, c) in output.export.class_constructors.iter() {
        let owner = &output.export.classes[c.owner];
        if !selected(&owner.name) {
            continue;
        }
        let names = c
            .parameters
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>()
            .join(",");
        let required = c
            .no_gc_type_params
            .iter()
            .map(|id| {
                owner
                    .type_params
                    .iter()
                    .find(|p| p.id == *id)
                    .unwrap()
                    .name
                    .as_str()
            })
            .collect::<Vec<_>>();
        rows.push(format!(
            "source {}({names}): Managed; requires {required:?}\n",
            owner.name
        ));
    }
    for (_, c) in output.local.struct_constructors.iter() {
        let owner = &output.local.structs[c.structure];
        if !selected(&owner.name) {
            continue;
        }
        let effect = match c.kind {
            hir::concrete::StructConstructorKind::Primary => hir::GcEffect::NoGc,
            hir::concrete::StructConstructorKind::Secondary { gc_effect, .. } => gc_effect,
        };
        if matches!(
            c.kind,
            hir::concrete::StructConstructorKind::Secondary {
                gc_effect: hir::GcEffect::NoGc,
                ..
            }
        ) {
            assert!(owner.gc_free);
            assert!(
                c.parameters
                    .iter()
                    .all(|p| output.local.types[p.ty].gc_free)
            );
        }
        let names = c
            .parameters
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>()
            .join(",");
        rows.push(format!(
            "concrete {}({names}): {effect:?}; result-gc-free {}\n",
            owner.name, owner.gc_free
        ));
    }
    for (_, f) in mir.functions.iter() {
        let Some((name, _)) = f.name.split_once(".$c") else {
            continue;
        };
        let owner = name
            .strip_prefix("ctor.")
            .or_else(|| name.strip_prefix("init."))
            .unwrap();
        if selected(owner) {
            rows.push(format!(
                "mir {name}/{}: {:?}\n",
                f.params.len(),
                f.gc_effect
            ));
        }
    }
    rows.sort();
    rows.concat()
}
