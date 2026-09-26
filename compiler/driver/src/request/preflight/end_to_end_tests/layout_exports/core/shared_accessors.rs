use super::*;

pub(super) fn check(
    name: &str,
    input: scoop_mir_lower::MirTypeBridgeExportInputV1<'_>,
    bridge: &mir::MirTypeBridgeExportConstituentsV1,
) {
    let cases: &[(&str, bool)] = match name {
        "shared-accessors-standalone" => &[
            ("AccessorCell.$get$stored", true),
            ("AccessorCell.$set$stored", true),
            ("AccessorCell.$get$computed", true),
            ("AccessorCell.$get$mixed", true),
            ("AccessorCell.$set$mixed", true),
        ],
        "shared-accessors-combined" => &[
            ("$get$accessorMarker", false),
            ("AccessorView.$get$slot", true),
            ("AccessorView.$set$slot", true),
            ("AccessorView.$get$defaulted", true),
            ("AccessorBase.$get$protectedStorage", false),
            ("AccessorBase.$set$protectedStorage", false),
            ("AccessorBase.$get$protectedBody", true),
            ("AccessorBase.$get$hidden", true),
            ("AccessorBase.$get$mixed", true),
            ("AccessorBase.$set$mixed", true),
            ("AccessorImpl.$get$slot", true),
            ("AccessorImpl.$set$slot", true),
            ("AccessorObject.$get$objectStorage", true),
            ("AccessorObject.$get$computed", true),
            ("$get$accessorGlobal", true),
            ("$set$accessorGlobal", true),
        ],
        _ => return,
    };
    let names = bridge
        .callables()
        .entries()
        .iter()
        .map(|binding| binding.implementation())
        .chain(
            input
                .ordinary
                .exports()
                .iter()
                .map(|record| record.implementation()),
        )
        .map(|implementation| {
            let root = input
                .mir
                .materialization()
                .callable_roots()
                .iter()
                .find(|root| root.implementation() == implementation.callable_owner())
                .unwrap();
            input.mir.module().functions[root.function()].name.as_str()
        })
        .collect::<Vec<_>>();
    for &(name, present) in cases {
        assert_eq!(names.contains(&name), present, "source callable {name}");
    }
}
