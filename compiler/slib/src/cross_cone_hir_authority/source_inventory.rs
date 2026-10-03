//! Checks direct references between complete shared declaration tables.

use super::*;
use scoop_hir::{NestedSourceMemberRefV1, NominalSourceShapeV1, PropertyDeclarationId};

mod errors;
pub use errors::{CrossConeHirSourceInventoryError, SourceInventoryDeclaration};
type Error = CrossConeHirSourceInventoryError;
type Declaration = SourceInventoryDeclaration;

impl CanonicalCrossConeHirSurfaceAuthority<'_> {
    pub(crate) fn validate_shared_source_inventory(&self) -> Result<(), Error> {
        let interface = self.current_interface;
        let callable = |id| {
            interface
                .callable_interfaces()
                .declaration(id)
                .map(|_| ())
                .ok_or(Error::Missing(Declaration::Callable(id)))
        };
        for record in interface.nominal_interfaces().all_records() {
            let details = record.declaration_details();
            for constructor in details.constructors().values() {
                callable(CallableTemplateOrigin::Constructor(*constructor))?;
            }
            for member in details.members().values() {
                match *member {
                    NestedSourceMemberRefV1::Function(id) => {
                        callable(CallableTemplateOrigin::Function(id))?
                    }
                    NestedSourceMemberRefV1::GenericFunction(id) => {
                        callable(CallableTemplateOrigin::GenericFunction(id))?
                    }
                    NestedSourceMemberRefV1::Property(id) => {
                        let id = PropertyOwner::Property(id);
                        if interface.property_interfaces().declaration(id).is_none() {
                            return Err(Error::Missing(Declaration::Property(id)));
                        }
                    }
                }
            }
            for child in details.children().values() {
                if interface.nominal_interfaces().declaration(*child).is_none() {
                    return Err(Error::Missing(Declaration::Nominal(*child)));
                }
            }
            if let NominalSourceShapeV1::Enum(shape) = record.source_shape() {
                for variant in shape.variants() {
                    callable(CallableTemplateOrigin::VariantConstructor(
                        variant.variant(),
                    ))?;
                }
            }
        }
        for record in interface.property_interfaces().all_declarations() {
            for accessor in
                std::iter::once(record.accessors().getter()).chain(record.accessors().setter())
            {
                callable(CallableTemplateOrigin::Accessor(accessor))?;
            }
        }
        Ok(())
    }
}
