use super::*;

mod enumeration;

impl Projection<'_, '_> {
    pub(super) fn value(
        &mut self,
        identity: lir::ExactLayoutIdentityV1,
        source: &mir::ParamFreeMirTypeExportV1,
        depth: u64,
    ) -> Result<lir::ExactValueLayoutV1> {
        use mir::{MirParamFreeIntrinsicV1 as Intrinsic, MirTypeRepresentationV1 as Kind};
        let foundation = self.output.foundation();
        Ok(match source.representation() {
            Kind::Intrinsic(Intrinsic::Unit) => {
                lir::ExactValueLayoutV1::unit(identity, foundation, self.meter)?
            }
            Kind::Intrinsic(Intrinsic::Integer(kind)) => lir::ExactValueLayoutV1::scalar(
                identity,
                lir::ScalarRepresentationKindV1::Integer(crate::metadata::integer_kind(*kind)),
                foundation,
                self.meter,
            )?,
            Kind::Intrinsic(Intrinsic::Boolean) => lir::ExactValueLayoutV1::scalar(
                identity,
                lir::ScalarRepresentationKindV1::Boolean,
                foundation,
                self.meter,
            )?,
            Kind::Intrinsic(Intrinsic::String)
            | Kind::Class { .. }
            | Kind::Interface
            | Kind::Object { .. }
            | Kind::ObjectBacking { .. }
            | Kind::BoxedValue { .. } => lir::ExactValueLayoutV1::qualified_pointer(
                identity,
                lir::NichePointerKind::Managed,
                foundation,
                self.meter,
            )?,
            Kind::Struct {
                fields,
                c_layout,
                interior_mutable,
            } => {
                let fields = self.fields(fields, depth)?;
                let fields = fields.inputs(self.meter)?;
                match c_layout {
                    mir::MirTypeCLayoutPolicyV1::Ordinary => {
                        lir::ExactValueLayoutV1::ordinary_struct(
                            identity,
                            *interior_mutable,
                            &fields,
                            foundation,
                            self.meter,
                        )?
                    }
                    mir::MirTypeCLayoutPolicyV1::CLayout(_) => {
                        self.meter.charge_work(
                            foundation.c_abi_layouts().len() as u64,
                            &WirePath::root(),
                        )?;
                        let contract = foundation
                            .c_abi_layouts()
                            .iter()
                            .find(|layout| layout.layout().exact_type() == source.exact())
                            .ok_or(ExactLayoutLoweringError::MissingCLayout(source.exact()))?;
                        lir::ExactValueLayoutV1::c_struct(
                            identity,
                            *interior_mutable,
                            &fields,
                            contract,
                            foundation,
                            self.meter,
                        )?
                    }
                }
            }
            Kind::Enum { variants }
            | Kind::CoroutineStep { variants }
            | Kind::CoroutineSlot { variants } => self.enumeration(identity, variants, depth)?,
        })
    }
}
