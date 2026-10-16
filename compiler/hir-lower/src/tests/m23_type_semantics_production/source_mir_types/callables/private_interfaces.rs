use super::*;

#[test]
fn private_interface_body_has_a_provider_binding_without_local_calls() {
    with_production(
        source_dispatch::INTERFACES,
        |output, input, hir, graph, _| {
            let local = output.output().local.module();
            let helper = local
                .functions
                .iter()
                .find(|(_, function)| function.name == "Root.helper")
                .expect("an ordinary private interface method has a provider body")
                .1;
            let scoop_identity::CallableTemplateOwner::Function(declaration) =
                helper.materialization.template()
            else {
                panic!("the helper retains its source function identity");
            };
            let types = complete_type_exports(output, input, hir, graph);
            let unit = dependencies::unit(input, graph);
            let index = MirTypeBridgeTypeIndexV1::try_new(&[&types, &unit]).unwrap();
            let bindings = lower_source_callable_bindings(
                output,
                &public_interface(output),
                hir,
                input,
                graph,
                &index,
                &[],
            )
            .unwrap();
            assert!(bindings.entries().iter().any(|binding| {
                *binding.origin() == scoop_mir::MirCallableOriginV1::Function(declaration)
            }));
        },
    );
}
