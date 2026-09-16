mod expansion;
mod record;
mod table;
mod target;

pub use expansion::{
    CanonicalTypeAliasExpansionsV1, TypeAliasClosureAuthority, TypeAliasExpansionError,
    TypeAliasExpansionV1,
};
pub use record::{
    DecodedTypeAliasInterfaceRecordV1, TypeAliasDeclarationSourceV1,
    TypeAliasInterfaceRecordBuildError, TypeAliasInterfaceRecordResolutionError,
    TypeAliasInterfaceRecordResolver, TypeAliasInterfaceRecordV1,
    TypeAliasInterfaceSemanticAuthority, TypeAliasInterfaceSemanticValidationError,
};
pub use table::{
    CanonicalTypeAliasInterfacesV1, DecodedCanonicalTypeAliasInterfacesV1,
    TypeAliasInterfaceSetBuildError, TypeAliasInterfaceSetSemanticValidationError,
    TypeAliasInterfaceSetValidationError,
};
pub use target::{DecodedTypeAliasTargetV1, TypeAliasTargetResolutionError, TypeAliasTargetV1};
