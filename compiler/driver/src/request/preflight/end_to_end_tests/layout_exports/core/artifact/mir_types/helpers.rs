use super::*;
use mir::MirTypeRepresentationV1 as Repr;

pub(super) fn check(replay: &Replay<'_, '_>, diamond: bool) {
    let interface = replay
        .section
        .types()
        .records()
        .iter()
        .find(|record| matches!(record.representation(), Repr::Interface))
        .unwrap()
        .exact();
    let helper = replay
        .section
        .types()
        .records()
        .iter()
        .find(|record| matches!(record.representation(), Repr::CoroutineStep { .. }))
        .unwrap();
    let mut bases = helper.base_and_interfaces().clone();
    bases.interfaces.push(interface);
    reject(replay, helper, bases);

    if diamond {
        let (helper, schema) = replay
            .section
            .types()
            .records()
            .iter()
            .find_map(|record| {
                let Repr::BoxedValue { payload } = record.representation() else {
                    return None;
                };
                let schema = replay.section.dispatch().get(payload.value)?;
                (schema.itables().len() > record.base_and_interfaces().interfaces.len())
                    .then_some((record, schema))
            })
            .unwrap();
        assert_eq!(helper.base_and_interfaces().interfaces.len(), 1);
        assert_eq!(schema.itables().len(), 4);
        let mut bases = helper.base_and_interfaces().clone();
        bases.interfaces = schema
            .itables()
            .iter()
            .map(mir::MirInterfaceDispatchTableV1::interface)
            .collect();
        reject(replay, helper, bases);
    }
}

fn reject(
    replay: &Replay<'_, '_>,
    helper: &mir::ParamFreeMirTypeExportV1,
    bases: mir::MirBaseAndInterfacesV1,
) {
    component(
        replay.reject(replay.replace(
            helper,
            helper.representation().clone(),
            helper.facts(),
            bases,
        )),
        Component::Interfaces,
        helper.exact(),
    );
}
