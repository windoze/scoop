use scoop_identity::CallableDefinitionOwner;

#[derive(Debug)]
pub enum CallableAbiProjectionError {
    MissingMirSignature(CallableDefinitionOwner),
    MirSignature(CallableDefinitionOwner),
    MissingMirBody(CallableDefinitionOwner),
    MissingLirBody(CallableDefinitionOwner),
    GcEffect(CallableDefinitionOwner),
    CallingConvention(CallableDefinitionOwner),
    ArgumentCount {
        target: CallableDefinitionOwner,
        mir: usize,
        lir: usize,
    },
    Identity(scoop_wire::HashError),
    Abi {
        target: CallableDefinitionOwner,
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
