use super::*;
pub(super) use crate::cross_cone_interface::expression_test_support::Fixture;
use scoop_identity::{
    CallableTemplateOrigin, LocalValueSelector, StructuralDefinitionPath,
    StructuralDefinitionSiteRole, StructuralPathSegment,
};

pub(super) fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}
pub(super) fn template(f: &Fixture, position: u32) -> ProtectedDefaultTemplateV1 {
    let owner = CallableTemplateOrigin::Function(f.function);
    let locals = (0..position)
        .map(|declaration_index| {
            TemplateLocalRecordV1::try_new(
                LocalValueSelector::Parameter { declaration_index },
                f.value_type(),
                CanonicalBooleanV1::False,
                TemplateLocalDefinitionV1::Source(f.origin()),
            )
            .unwrap()
        })
        .collect();
    let parameters = (0..position)
        .map(|index| {
            TemplateValueParameterV1::try_new(
                index,
                LocalValueSelector::Parameter {
                    declaration_index: index,
                },
            )
            .unwrap()
        })
        .collect();
    ProtectedDefaultTemplateV1::try_new(
        ProtectedDefaultTemplateKeyV1::try_new(owner, position).unwrap(),
        PersistentLexicalRootV1::Function(f.function),
        StructuralDefinitionPath::from_first(
            StructuralPathSegment::new(StructuralDefinitionSiteRole::DefaultValue, position),
            [],
        ),
        CanonicalTemplateLocalTableV1::try_new(locals).unwrap(),
        ExportDefaultBodyV1::try_new(
            vec![],
            DefaultExpressionV1::try_new(
                DefaultExpressionKindV1::Local(f.local()),
                f.value_type(),
                f.origin(),
            )
            .unwrap(),
        )
        .unwrap(),
        f.value_type(),
        CanonicalBooleanV1::False,
        CanonicalBinderUseListV1::try_new(vec![]).unwrap(),
        OptionalTemplateReceiverV1::Absent,
        CanonicalTemplateValueParametersV1::try_new(parameters).unwrap(),
        ProtectedDefaultReferenceSetV1::try_new(vec![], vec![], vec![], vec![], vec![], vec![])
            .unwrap(),
        f.origin(),
    )
    .unwrap()
}
pub(super) struct Raw<'a>(pub &'a [&'a ProtectedDefaultTemplateV1]);
impl WireEncode for Raw<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0.len() as u64)?;
        for template in self.0 {
            template.index_locals().unwrap().encode(encoder)?;
        }
        Ok(())
    }
}
pub(super) fn decoded(value: &impl WireEncode) -> DecodedCanonicalProtectedDefaultTemplatesV1 {
    decode_canonical(&encode(value).unwrap(), DecodeLimits::default()).unwrap()
}
