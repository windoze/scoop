use super::*;

mod enumeration;

impl Projection<'_> {
    pub(super) fn value(
        &mut self,
        identity: lir::ExactLayoutIdentityV1,
        source: &mir::ParamFreeMirTypeExportV1,
    ) -> Result<lir::ExactValueLayoutV1> {
        use mir::{MirParamFreeIntrinsicV1 as Intrinsic, MirTypeRepresentationV1 as Kind};
        let foundation = self.output.foundation();
        Ok(match source.representation() {
            Kind::Intrinsic(Intrinsic::Unit) => {
                lir::ExactValueLayoutV1::unit(identity, foundation)?
            }
            Kind::Intrinsic(Intrinsic::Integer(kind)) => lir::ExactValueLayoutV1::scalar(
                identity,
                lir::ScalarRepresentationKindV1::Integer(crate::metadata::integer_kind(*kind)),
                foundation,
            )?,
            Kind::Intrinsic(Intrinsic::Float(kind)) => lir::ExactValueLayoutV1::scalar(
                identity,
                lir::ScalarRepresentationKindV1::Float(*kind),
                foundation,
            )?,
            Kind::Intrinsic(Intrinsic::Char) => lir::ExactValueLayoutV1::scalar(
                identity,
                lir::ScalarRepresentationKindV1::Char,
                foundation,
            )?,
            Kind::Intrinsic(Intrinsic::Boolean) => lir::ExactValueLayoutV1::scalar(
                identity,
                lir::ScalarRepresentationKindV1::Boolean,
                foundation,
            )?,
            Kind::Intrinsic(
                Intrinsic::String
                | Intrinsic::Any
                | Intrinsic::Nothing
                | Intrinsic::AtomicInt
                | Intrinsic::AtomicLong
                | Intrinsic::AtomicBoolean,
            )
            | Kind::Class { .. }
            | Kind::InlineArray { .. }
            | Kind::AtomicReference { .. }
            | Kind::Object { .. }
            | Kind::ObjectBacking { .. }
            | Kind::BoxedValue { .. } => lir::ExactValueLayoutV1::qualified_pointer(
                identity,
                lir::NullNicheKind::Managed,
                foundation,
            )?,
            Kind::Interface => lir::ExactValueLayoutV1::interface(identity, foundation)?,
            Kind::Struct {
                fields,
                c_layout,
                interior_mutable,
            } => {
                let fields = self.fields(fields)?;
                let fields = fields.inputs()?;
                match c_layout {
                    mir::MirTypeCLayoutPolicyV1::Ordinary => {
                        lir::ExactValueLayoutV1::ordinary_struct(
                            identity,
                            *interior_mutable,
                            &fields,
                            foundation,
                        )?
                    }
                    mir::MirTypeCLayoutPolicyV1::CLayout(_) => {
                        let contract = foundation
                            .c_abi_layouts()
                            .iter()
                            .find(|layout| layout.layout().exact_type() == source.exact())
                            .ok_or(ExactLayoutLoweringError::MissingCLayout(source.exact()))?;
                        let mut dependencies = self.reserve(fields.len())?;
                        for field in contract.layout().fields() {
                            if let scoop_identity::CanonicalCStorageType::Struct {
                                exact_type,
                                ..
                            } = field.storage()
                            {
                                dependencies.push(self.value_dependency(exact_type)?);
                            }
                        }
                        let mut nested = self.reserve(dependencies.len())?;
                        nested.extend(dependencies.iter().map(AsRef::as_ref));
                        lir::ExactValueLayoutV1::c_struct(
                            identity,
                            *interior_mutable,
                            &fields,
                            contract,
                            &nested,
                            foundation,
                        )?
                    }
                }
            }
            Kind::Enum { variants }
            | Kind::CoroutineStep { variants }
            | Kind::CoroutineSlot { variants } => self.enumeration(identity, variants)?,
        })
    }
}
