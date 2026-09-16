mod record;
mod target;

pub use record::{
    DecodedTypeAliasInterfaceRecordV1, TypeAliasDeclarationSourceV1,
    TypeAliasInterfaceRecordBuildError, TypeAliasInterfaceRecordResolutionError,
    TypeAliasInterfaceRecordResolver, TypeAliasInterfaceRecordV1,
    TypeAliasInterfaceSemanticAuthority, TypeAliasInterfaceSemanticValidationError,
};
pub use target::{DecodedTypeAliasTargetV1, TypeAliasTargetResolutionError, TypeAliasTargetV1};
