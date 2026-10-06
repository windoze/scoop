use super::*;
use crate::{ClassLayoutBaseV1, NominalLayoutFieldInputV1};
use scoop_identity::{ExactTypeKey, GeneratedNominalKey, PersistentTypeId};

impl LayoutReader<'_> {
    pub(super) fn instance(
        &mut self,
        identity: ExactLayoutIdentityV1,
        raw: &RawInstance,
    ) -> Result<ExactInstanceLayoutV1, LinkDataError> {
        let result = match raw {
            RawInstance::InlineBytes => {
                ExactInstanceLayoutV1::inline_bytes(identity, self.foundation)
            }
            RawInstance::AbstractReference => {
                ExactInstanceLayoutV1::abstract_reference(identity, self.foundation)
            }
            RawInstance::Box { layout, .. } => {
                let layout = self.identities.resolve(*layout).map_err(link_error)?;
                ExactInstanceLayoutV1::boxed_payload(
                    identity,
                    self.value_by_id(layout)?.as_ref(),
                    self.foundation,
                )
            }
            RawInstance::InlineArray { exact, .. } => {
                let exact = self.identities.resolve(*exact).map_err(link_error)?;
                let layout = value_layout_id(self.target, exact)?;
                ExactInstanceLayoutV1::inline_array(
                    identity,
                    self.value_by_id(layout)?.as_ref(),
                    self.foundation,
                )
            }
            RawInstance::Class { base, declared, .. } => {
                let base = match base {
                    RawBase::NoBase => None,
                    RawBase::Prefix { layout, .. } => {
                        let id = self.identities.resolve(*layout).map_err(link_error)?;
                        Some(
                            self.layouts
                                .get(&id)
                                .and_then(ExactLayoutExportV1::instance_handle)
                                .ok_or_else(|| {
                                    LinkDataError(format!("missing base instance layout {id}"))
                                })?,
                        )
                    }
                };
                let base = base
                    .as_deref()
                    .map_or(ClassLayoutBaseV1::NoBase, ClassLayoutBaseV1::Base);
                let fields = self.nominal_fields(declared)?;
                let fields = fields
                    .iter()
                    .map(|(field, value)| NominalLayoutFieldInputV1 {
                        field,
                        value: &value.value,
                    })
                    .collect::<Vec<_>>();
                let backing_key = match identity.exact_key() {
                    ExactTypeKey::Nominal(object) => {
                        Some(GeneratedNominalKey::ObjectBackingClass { object: *object })
                    }
                    ExactTypeKey::NominalApplication { origin, .. } => {
                        Some(GeneratedNominalKey::GenericObjectBackingClass { object: *origin })
                    }
                    _ => None,
                };
                let backing = if let Some(key) = backing_key {
                    let id = PersistentTypeId::from_generated_key(&key).map_err(link_error)?;
                    if self.identities.contains_resolved_identity(id) {
                        Some(self.identities.canonical_record(id).map_err(link_error)?)
                    } else {
                        None
                    }
                } else {
                    None
                };
                match backing {
                    Some(backing) => ExactInstanceLayoutV1::object(
                        identity,
                        &backing,
                        base,
                        &fields,
                        self.foundation,
                    ),
                    None => ExactInstanceLayoutV1::class(identity, base, &fields, self.foundation),
                }
            }
        };
        result.map_err(link_error)
    }
}
