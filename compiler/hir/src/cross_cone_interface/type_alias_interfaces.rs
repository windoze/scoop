mod record;
mod target;

pub use record::{
    DecodedTypeAliasInterfaceRecordV1, TypeAliasInterfaceRecordBuildError,
    TypeAliasInterfaceRecordResolutionError, TypeAliasInterfaceRecordResolver,
    TypeAliasInterfaceRecordV1,
};
pub use target::{DecodedTypeAliasTargetV1, TypeAliasTargetResolutionError, TypeAliasTargetV1};
