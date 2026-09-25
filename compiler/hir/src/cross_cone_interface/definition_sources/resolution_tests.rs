use super::*;
use scoop_identity::{
    ConeCoordinate, DecodedPersistentId, NormalizedSourcePath, SourceIdentity,
    SourceIdentityResolutionError, SourceOriginError, SourceSpan,
};
use scoop_wire::{decode_canonical, encode};
use std::sync::Arc;

mod canonical;

mod support;
use support::*;
