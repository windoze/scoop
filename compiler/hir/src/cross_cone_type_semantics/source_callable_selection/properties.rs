use super::*;
use scoop_identity::PropertyOwner;

impl Selection<'_, '_, '_> {
    pub(super) fn properties(&mut self) -> Result<(), Error> {
        for property in self.public.property_interfaces().all_declarations() {
            lookup(self.properties.len(), self.meter)?;
            let mut selected = match property.declaration() {
                PropertyOwner::Property(id) => self.properties.remove(&id),
                PropertyOwner::ExtensionProperty(_) => false,
            };
            for accessor in
                std::iter::once(property.accessors().getter()).chain(property.accessors().setter())
            {
                lookup(self.required.len(), self.meter)?;
                selected |= self
                    .required
                    .contains_key(&Declaration::PropertyAccessor(accessor));
            }
            for source in std::iter::once(property.accessors().getter_source())
                .chain(property.accessors().setter_source())
            {
                let accessor = source.accessor();
                if !source.implementation().requires_body() {
                    lookup(self.required.len(), self.meter)?;
                    self.required
                        .remove(&Declaration::PropertyAccessor(accessor));
                } else if selected {
                    lookup(
                        self.public.callable_interfaces().declaration_count(),
                        self.meter,
                    )?;
                    let callable = self
                        .public
                        .callable_interfaces()
                        .declaration(Origin::Accessor(accessor))
                        .ok_or(Error::CallableContract(Origin::Accessor(accessor)))?;
                    if matches!(
                        callable.declared_visibility(),
                        crate::DeclaredVisibilityV1::Public
                            | crate::DeclaredVisibilityV1::Protected
                    ) {
                        self.insert(callable.declaration())?;
                    }
                }
            }
        }
        if let Some(&missing) = self.properties.first() {
            return Err(Error::PropertyContract(missing));
        }
        Ok(())
    }
}
