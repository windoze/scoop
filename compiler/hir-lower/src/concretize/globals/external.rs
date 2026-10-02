use super::*;
use scoop_identity::{
    PersistentPropertyId, PropertyOwner, SourceNativeExternalContract,
    SourceNativeExternalContractRecord, SourceNativeLibraryBinding,
};

impl Concretizer<'_> {
    pub(in crate::concretize) fn lower_external_global(
        &mut self,
        property: PersistentPropertyId,
        source: &SourceNativeExternalContractRecord,
        ty: concrete::TypeId,
        span: export::Span,
    ) -> concrete::GlobalId {
        let owner = concrete::PropertyStorageOwner::Backing(PropertyOwner::Property(property));
        if let Some((id, _)) = self
            .globals
            .iter()
            .find(|(_, value)| value.storage_owner == owner)
        {
            return id;
        }
        let (symbol, library, mutable, thread_local) = match source.contract() {
            SourceNativeExternalContract::ReadOnlyData {
                symbol, library, ..
            } => (symbol, library, false, false),
            SourceNativeExternalContract::MutableData {
                symbol, library, ..
            } => (symbol, library, true, false),
            SourceNativeExternalContract::ReadOnlyTls {
                symbol, library, ..
            } => (symbol, library, false, true),
            SourceNativeExternalContract::MutableTls {
                symbol, library, ..
            } => (symbol, library, true, true),
            SourceNativeExternalContract::Function { .. } => {
                unreachable!("an external global place retains a property contract")
            }
        };
        let library = match library {
            SourceNativeLibraryBinding::DefaultNativeNamespace => String::new(),
            SourceNativeLibraryBinding::LogicalLibrary(name) => name.as_str().to_owned(),
        };
        let symbol =
            std::str::from_utf8(symbol.as_bytes()).expect("source symbols retain validated UTF-8");
        self.globals.alloc(concrete::Global {
            name: symbol.to_owned(),
            storage_owner: owner,
            ty,
            mutable,
            storage: concrete::GlobalStorage::Extern {
                source_contract: Box::new(source.clone()),
                library,
                native_symbol: symbol.to_owned(),
                thread_local,
            },
            span,
        })
    }
}
