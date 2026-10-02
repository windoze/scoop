use std::fmt::Write;

use crate::LocalConcreteHirOutput;

/// Dumps only the concrete graph, in arena order, without lookup-map order or
/// borrowed dependency worlds. Persistent identities and expression origins
/// remain visible alongside the distinct local entity ids.
pub fn dump_local(output: &LocalConcreteHirOutput) -> String {
    let module = output.module();
    let mut text = format!("LocalConcrete {} {:?}\n", module.cone, output.output_kind());
    for (id, ty) in module.types.iter() {
        writeln!(
            text,
            "  type {id:?} {ty:?} {:?}",
            module.exact_type_identities[id]
        )
        .expect("writing to String");
    }
    macro_rules! arena {
        ($($field:ident),* $(,)?) => {$(
            for (id, value) in module.$field.iter() {
                writeln!(text, "  {} {id:?} {value:?}", stringify!($field))
                    .expect("writing to String");
            }
        )*};
    }
    arena!(
        function_types,
        lambdas,
        anonymous_functions,
        local_functions,
        callable_references,
        imported_dependency_callables,
        function_coercions,
        foreign_callback_registrations,
        functions,
        extern_functions,
        globals,
        generic_delegate_specializations,
        initialization_units,
        initialization_failure_roots,
        objects,
        object_types,
        companion_relations,
        singleton_values,
        singleton_published_roots,
        structs,
        enums,
        classes,
        class_constructors,
        struct_constructors,
        interfaces,
    );
    writeln!(
        text,
        "  generated {:?}",
        module.generated_callable_identities
    )
    .expect("writing to String");
    writeln!(text, "  callbacks {:?}", module.native_callback_signatures)
        .expect("writing to String");
    writeln!(text, "  coroutines {:?}", module.coroutine_protocols).expect("writing to String");
    text
}
