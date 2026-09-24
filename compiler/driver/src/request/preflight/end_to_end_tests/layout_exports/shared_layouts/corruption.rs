use super::*;
use scoop_identity::{PersistentExactTypeId, RepresentationRole};
use scoop_wire::Encoder;

pub(super) fn check(
    input: LayoutAbiExportInputV1<'_>,
    layouts: &lir::CanonicalExactLayoutExportsV1,
) {
    let records = layouts.records();
    let reject = |rows: Vec<lir::ExactLayoutExportV1>| {
        let wire: lir::DecodedCanonicalExactLayoutExportsV1 = decoded(&Rows(&rows));
        assert!(wire.validate_against(layouts, &mut meter()).is_err());
    };
    for index in 0..records.len() {
        let mut missing = records.to_vec();
        missing.remove(index);
        reject(missing);
    }
    let mut duplicate = records.to_vec();
    duplicate[1] = duplicate[0].clone();
    reject(duplicate);
    let mut reversed = records.to_vec();
    reversed.reverse();
    reject(reversed);
    let mut extra = records.to_vec();
    extra.push(records[0].clone());
    reject(extra);

    let source = named(input, "SharedLayoutPair");
    let mir::MirTypeRepresentationV1::Struct {
        fields,
        c_layout,
        interior_mutable,
    } = source.representation()
    else {
        panic!("the fixture contains a source struct");
    };
    let mut recursive = fields.clone();
    recursive[0].value = source.exact();
    let types = replace(
        input,
        source,
        mir::MirTypeRepresentationV1::Struct {
            fields: recursive,
            c_layout: *c_layout,
            interior_mutable: *interior_mutable,
        },
    );
    let layout = layouts
        .find_exact_role(source.exact(), RepresentationRole::ManagedValue)
        .unwrap();
    assert!(matches!(
        replay(input, &types, &[], &mut meter()),
        Err(Error::Cycle(id)) if id == layout.identity().layout()
    ));

    let mut changed = fields.clone();
    changed[0].value = input
        .bridge
        .types()
        .records()
        .iter()
        .find_map(|record| {
            matches!(
                record.representation(),
                mir::MirTypeRepresentationV1::Intrinsic(mir::MirParamFreeIntrinsicV1::String)
            )
            .then_some(record.exact())
        })
        .unwrap();
    let types = replace(
        input,
        source,
        mir::MirTypeRepresentationV1::Struct {
            fields: changed,
            c_layout: *c_layout,
            interior_mutable: *interior_mutable,
        },
    );
    assert!(matches!(
        replay(input, &types, &[], &mut meter()),
        Err(Error::SourceFacts(exact)) if exact == source.exact()
    ));

    for source in input.bridge.types().records() {
        let mir::MirTypeRepresentationV1::Struct {
            fields,
            c_layout: mir::MirTypeCLayoutPolicyV1::CLayout(policy),
            interior_mutable,
        } = source.representation()
        else {
            continue;
        };
        let mut policy = *policy;
        policy.packed = if policy.packed == mir::MirCLayoutValue::Natural {
            mir::MirCLayoutValue::A1
        } else {
            mir::MirCLayoutValue::Natural
        };
        let types = replace(
            input,
            source,
            mir::MirTypeRepresentationV1::Struct {
                fields: fields.clone(),
                c_layout: mir::MirTypeCLayoutPolicyV1::CLayout(policy),
                interior_mutable: *interior_mutable,
            },
        );
        assert!(matches!(
            replay(input, &types, &[], &mut meter()),
            Err(Error::CLayout(exact)) if exact == source.exact()
        ));
    }
}

fn named<'a>(input: LayoutAbiExportInputV1<'a>, name: &str) -> &'a mir::ParamFreeMirTypeExportV1 {
    let exact: PersistentExactTypeId = input
        .lir
        .module()
        .meta
        .layouts
        .iter()
        .find(|(_, layout)| layout.name == name)
        .map(|(_, layout)| layout.identity.layout_record().key().exact_type())
        .unwrap();
    input.bridge.types().get(exact).unwrap()
}

fn replace(
    input: LayoutAbiExportInputV1<'_>,
    source: &mir::ParamFreeMirTypeExportV1,
    representation: mir::MirTypeRepresentationV1,
) -> mir::CanonicalParamFreeMirTypeExportsV1 {
    let changed = mir::ParamFreeMirTypeExportV1::try_new(
        mir::MirTypeBridgeAuthority {
            identities: input.identities,
            foundation: input.mir.foundation(),
        },
        source.exact(),
        source.origin().clone(),
        source.facts(),
        representation,
        source.base_and_interfaces().clone(),
    )
    .unwrap();
    mir::CanonicalParamFreeMirTypeExportsV1::try_new(
        input
            .bridge
            .types()
            .records()
            .iter()
            .map(|record| {
                if record.exact() == source.exact() {
                    changed.clone()
                } else {
                    record.clone()
                }
            })
            .collect(),
    )
    .unwrap()
}

struct Rows<'a>(&'a [lir::ExactLayoutExportV1]);
impl WireEncode for Rows<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0.len() as u64)?;
        for record in self.0 {
            record.encode(encoder)?;
        }
        Ok(())
    }
}
