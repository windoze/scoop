use scoop_identity::{GeneratedNominalKey, PersistentTypeId};

use super::*;

impl Replay<'_> {
    pub(super) fn instance(
        &mut self,
        identity: lir::ExactLayoutIdentityV1,
        source: &mir::ParamFreeMirTypeExportV1,
    ) -> Result<lir::ExactInstanceLayoutV1> {
        use mir::{MirParamFreeIntrinsicV1 as Intrinsic, MirTypeRepresentationV1 as Kind};
        let foundation = self.foundation;
        Ok(match source.representation() {
            Kind::Intrinsic(Intrinsic::String) => {
                lir::ExactInstanceLayoutV1::inline_bytes(identity, foundation)?
            }
            Kind::InlineArray { element } => {
                let element = self.value_dependency(*element)?;
                lir::ExactInstanceLayoutV1::inline_array(identity, &element, foundation)?
            }
            Kind::Interface | Kind::Intrinsic(Intrinsic::Any | Intrinsic::Nothing) => {
                lir::ExactInstanceLayoutV1::abstract_reference(identity, foundation)?
            }
            Kind::Class {
                declared_fields, ..
            }
            | Kind::ObjectBacking { declared_fields } => {
                let backing = if let mir::MirTypeOriginV1::NominalApplication(object) =
                    source.origin()
                {
                    let key = GeneratedNominalKey::GenericObjectBackingClass { object: *object };
                    let id = PersistentTypeId::from_generated_key(&key)
                        .map_err(|_| Error::SourceObject(source.exact()))?;
                    self.identities
                        .contains_resolved_identity(id)
                        .then(|| {
                            self.identities
                                .canonical_record::<PersistentTypeId, GeneratedNominalKey>(id)
                        })
                        .transpose()?
                } else {
                    None
                };
                self.class(identity, source, declared_fields, backing.as_ref())?
            }
            Kind::Object { backing } => {
                let backing_shape = self
                    .types
                    .get(*backing)
                    .ok_or(Error::MissingMirShape(*backing))?;
                let Kind::ObjectBacking { declared_fields } = backing_shape.representation() else {
                    return Err(Error::SourceObject(source.exact()));
                };
                let mir::MirTypeOriginV1::GeneratedNominal { nominal, .. } = backing_shape.origin()
                else {
                    return Err(Error::SourceObject(source.exact()));
                };
                let backing = self
                    .identities
                    .canonical_record::<_, GeneratedNominalKey>(*nominal)?;
                self.class(identity, backing_shape, declared_fields, Some(&backing))?
            }
            Kind::BoxedValue { payload } => {
                let payload = self.value_dependency(payload.value)?;
                lir::ExactInstanceLayoutV1::boxed_payload(identity, &payload, foundation)?
            }
            Kind::Intrinsic(
                Intrinsic::Unit
                | Intrinsic::Integer(_)
                | Intrinsic::Float(_)
                | Intrinsic::Char
                | Intrinsic::Boolean,
            )
            | Kind::Struct { .. }
            | Kind::Enum { .. }
            | Kind::CoroutineStep { .. }
            | Kind::CoroutineSlot { .. } => {
                let payload = self.value_dependency(source.exact())?;
                lir::ExactInstanceLayoutV1::boxed_payload(identity, &payload, foundation)?
            }
        })
    }

    fn class(
        &mut self,
        identity: lir::ExactLayoutIdentityV1,
        source: &mir::ParamFreeMirTypeExportV1,
        fields: &[mir::MirRepresentationFieldV1],
        backing: Option<&CborIdentityRecord<PersistentTypeId, GeneratedNominalKey>>,
    ) -> Result<lir::ExactInstanceLayoutV1> {
        let base = match source.base_and_interfaces().base {
            mir::MirBaseClassV1::None => None,
            mir::MirBaseClassV1::Base(exact) => Some(
                self.layout(exact, RepresentationRole::ManagedObject)?
                    .instance_handle()
                    .ok_or(Error::DependencyKind(exact))?,
            ),
        };
        let base = match &base {
            None => lir::ClassLayoutBaseV1::NoBase,
            Some(base) => lir::ClassLayoutBaseV1::Base(base),
        };
        let fields = self.fields(fields)?;
        let fields = fields.inputs()?;
        Ok(match backing {
            Some(backing) => lir::ExactInstanceLayoutV1::object(
                identity,
                backing,
                base,
                &fields,
                self.foundation,
            )?,
            None => lir::ExactInstanceLayoutV1::class(identity, base, &fields, self.foundation)?,
        })
    }
}
