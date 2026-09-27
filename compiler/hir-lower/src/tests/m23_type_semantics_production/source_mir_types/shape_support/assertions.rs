use super::*;
use scoop_mir::{GeneratedExactTypeLocation, StrongBoxedShapeSupportRoot};
use std::fmt::Write;

pub(super) fn bindings(input: &scoop_mir::ConeMirInput, families: &CanonicalMirShapeSupportsV1) {
    let module = input.module();
    let roots = input.materialization().shape_support();
    assert_eq!(families.records().len(), roots.len());
    for root in roots {
        let family = families.get(root.shape().source()).unwrap();
        assert_eq!(family.exact(), root.shape().exact());
        assert_eq!(family.provider(), module.cone);
        assert_eq!(family.coroutine_step(), root.coroutine_step().exact());
        assert_eq!(family.coroutine_slot(), root.coroutine_slot().exact());
        let GeneratedExactTypeLocation::Enum(step) = root.coroutine_step().location() else {
            panic!("step is an enum")
        };
        let GeneratedExactTypeLocation::Enum(slot) = root.coroutine_slot().location() else {
            panic!("slot is an enum")
        };
        assert!(
            module
                .meta
                .coroutine_steps
                .iter()
                .any(|(_, actual)| actual.enum_id() == step
                    && actual.identity().result_record().id() == family.exact())
        );
        assert!(
            module
                .meta
                .coroutine_slots
                .iter()
                .any(|(_, actual)| actual.enum_id() == slot
                    && actual.identity().value_record().id() == family.exact())
        );
        let mut helpers = vec![root.coroutine_step(), root.coroutine_slot()];
        match (root.boxed(), family.boxed()) {
            (
                StrongBoxedShapeSupportRoot::Available(boxed),
                MirBoxedShapeSupportV1::Available(exact),
            ) => {
                assert_eq!(boxed.exact(), exact);
                let GeneratedExactTypeLocation::Class(class) = boxed.location() else {
                    panic!("box is a class")
                };
                assert!(
                    module
                        .meta
                        .boxed_types
                        .iter()
                        .any(|actual| actual.class() == class
                            && actual.payload() == root.shape().ty())
                );
                helpers.push(boxed);
            }
            (
                StrongBoxedShapeSupportRoot::ReferenceNominalRequiresNoBox,
                MirBoxedShapeSupportV1::ReferenceNominalRequiresNoBox,
            ) => {}
            _ => panic!("box binding matches the source representation"),
        }
        for helper in helpers {
            let actual = module
                .meta
                .generated_exact_types
                .get(helper.location())
                .unwrap();
            assert_eq!(actual.nominal_record().id(), helper.nominal());
            assert_eq!(actual.exact_record().id(), helper.exact());
        }
    }
}

pub(super) fn projection(
    output: &hir::DependencyHirOutput,
    families: &CanonicalMirShapeSupportsV1,
    types: &CanonicalParamFreeMirTypeExportsV1,
) -> String {
    let names = source_dispatch::owners(output);
    assert_eq!(names.len(), families.records().len());
    let mut text = String::new();
    for (name, exact) in names {
        let family = families
            .records()
            .iter()
            .find(|family| family.exact() == exact)
            .unwrap();
        let boxed = match family.boxed() {
            MirBoxedShapeSupportV1::Available(_) => "available",
            MirBoxedShapeSupportV1::ReferenceNominalRequiresNoBox => "unneeded",
        };
        writeln!(
            text,
            "{name}: box={boxed} step={:?} slot={:?}",
            types.get(family.coroutine_step()).unwrap().facts().gc(),
            types.get(family.coroutine_slot()).unwrap().facts().gc()
        )
        .unwrap();
    }
    text
}

pub(super) fn hidden_box(input: &scoop_mir::ConeMirInput, families: &CanonicalMirShapeSupportsV1) {
    let module = input.module();
    let hidden = module.meta.boxed_types.iter().find(|boxed| matches!(boxed.payload(), scoop_mir::Type::Struct(id) if module.structs[*id].name == "Hidden")).unwrap();
    let exact = module
        .meta
        .source_exact_types
        .get(hidden.payload())
        .unwrap()
        .identity_record()
        .id();
    assert!(
        families
            .records()
            .iter()
            .all(|family| family.exact() != exact)
    );
}
