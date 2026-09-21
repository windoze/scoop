use scoop_identity::StrongCallableDefinitionOwner;

#[derive(Debug)]
pub enum CallableAbiProjectionError {
    MissingMirSignature(StrongCallableDefinitionOwner),
    MirSignature(StrongCallableDefinitionOwner),
    MissingMirBody(StrongCallableDefinitionOwner),
    MissingLirBody(StrongCallableDefinitionOwner),
    GcEffect(StrongCallableDefinitionOwner),
    CallingConvention(StrongCallableDefinitionOwner),
    ArgumentCount {
        target: StrongCallableDefinitionOwner,
        mir: usize,
        lir: usize,
    },
    Identity(scoop_wire::HashError),
    Abi {
        target: StrongCallableDefinitionOwner,
        source: scoop_lir::CallableAbiBuildError,
    },
}

impl std::fmt::Display for CallableAbiProjectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "cannot project materialized callable ABI: {self:?}"
        )
    }
}
impl std::error::Error for CallableAbiProjectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Identity(source) => Some(source),
            Self::Abi { source, .. } => Some(source),
            _ => None,
        }
    }
}
