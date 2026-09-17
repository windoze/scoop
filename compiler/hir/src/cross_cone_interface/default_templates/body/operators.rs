use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

macro_rules! wire_leaf_enum {
    (
        $(#[$attribute:meta])*
        pub enum $name:ident {
            $($variant:ident = $tag:literal),+ $(,)?
        }
    ) => {
        $(#[$attribute])*
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub enum $name {
            $($variant),+
        }

        impl WireEncode for $name {
            fn encode(
                &self,
                encoder: &mut Encoder,
            ) -> Result<(), scoop_wire::cbor::EncodeError> {
                encoder.unsigned(match self {
                    $(Self::$variant => $tag),+
                })
            }
        }

        impl WireDecode for $name {
            fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
                match decoder.unsigned()? {
                    $($tag => Ok(Self::$variant),)+
                    tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
                }
            }
        }
    };
}

wire_leaf_enum! {
    /// Width and signedness of an integer operation in a portable default.
    pub enum DefaultIntegerKindV1 {
        Signed8 = 1,
        Signed16 = 2,
        Signed32 = 3,
        Signed64 = 4,
        Unsigned8 = 5,
        Unsigned16 = 6,
        Unsigned32 = 7,
        Unsigned64 = 8,
    }
}

wire_leaf_enum! {
    pub enum DefaultNoGcIntegerOperationV1 {
        UnaryPlus = 1,
        UnaryMinus = 2,
        Inc = 3,
        Dec = 4,
        Add = 5,
        Sub = 6,
        Mul = 7,
        CompareTo = 8,
        Equals = 9,
        And = 10,
        Or = 11,
        Xor = 12,
        Inv = 13,
        Shl = 14,
        Shr = 15,
        Ushr = 16,
    }
}

wire_leaf_enum! {
    pub enum DefaultIntegerDivRemV1 {
        Div = 1,
        Rem = 2,
    }
}

wire_leaf_enum! {
    pub enum DefaultPrimitiveBinaryKindV1 {
        StringConcat = 1,
        StringCompareTo = 2,
    }
}

wire_leaf_enum! {
    pub enum DefaultPrimitiveUnaryKindV1 {
        BooleanNot = 1,
    }
}

wire_leaf_enum! {
    pub enum DefaultArrayAccessKindV1 {
        ImmutableGet = 1,
        MutableGet = 2,
        MutableSet = 3,
    }
}

wire_leaf_enum! {
    pub enum DefaultForeignCallbackOperationV1 {
        Retain = 1,
        Release = 2,
        State = 3,
        Failure = 4,
    }
}

wire_leaf_enum! {
    pub enum DefaultBinaryOperatorV1 {
        Lt = 1,
        Le = 2,
        Gt = 3,
        Ge = 4,
        RefEq = 5,
        RefNe = 6,
        And = 7,
        Or = 8,
    }
}

wire_leaf_enum! {
    pub enum DefaultUnaryOperatorV1 {
        Not = 1,
    }
}

impl From<crate::IntegerKind> for DefaultIntegerKindV1 {
    fn from(value: crate::IntegerKind) -> Self {
        match value {
            crate::IntegerKind::SIGNED_8 => Self::Signed8,
            crate::IntegerKind::SIGNED_16 => Self::Signed16,
            crate::IntegerKind::SIGNED_32 => Self::Signed32,
            crate::IntegerKind::SIGNED_64 => Self::Signed64,
            crate::IntegerKind::UNSIGNED_8 => Self::Unsigned8,
            crate::IntegerKind::UNSIGNED_16 => Self::Unsigned16,
            crate::IntegerKind::UNSIGNED_32 => Self::Unsigned32,
            crate::IntegerKind::UNSIGNED_64 => Self::Unsigned64,
        }
    }
}

impl From<DefaultIntegerKindV1> for crate::IntegerKind {
    fn from(value: DefaultIntegerKindV1) -> Self {
        match value {
            DefaultIntegerKindV1::Signed8 => Self::SIGNED_8,
            DefaultIntegerKindV1::Signed16 => Self::SIGNED_16,
            DefaultIntegerKindV1::Signed32 => Self::SIGNED_32,
            DefaultIntegerKindV1::Signed64 => Self::SIGNED_64,
            DefaultIntegerKindV1::Unsigned8 => Self::UNSIGNED_8,
            DefaultIntegerKindV1::Unsigned16 => Self::UNSIGNED_16,
            DefaultIntegerKindV1::Unsigned32 => Self::UNSIGNED_32,
            DefaultIntegerKindV1::Unsigned64 => Self::UNSIGNED_64,
        }
    }
}

macro_rules! bidirectional_mapping {
    (
        $wire:ty,
        $hir:ty,
        {$($wire_variant:ident => $hir_variant:ident),+ $(,)?}
    ) => {
        impl From<$hir> for $wire {
            fn from(value: $hir) -> Self {
                match value {
                    $(<$hir>::$hir_variant => Self::$wire_variant),+
                }
            }
        }

        impl From<$wire> for $hir {
            fn from(value: $wire) -> Self {
                match value {
                    $(<$wire>::$wire_variant => Self::$hir_variant),+
                }
            }
        }
    };
}

bidirectional_mapping!(
    DefaultNoGcIntegerOperationV1,
    crate::NoGcIntegerOperation,
    {
        UnaryPlus => UnaryPlus,
        UnaryMinus => UnaryMinus,
        Inc => Inc,
        Dec => Dec,
        Add => Add,
        Sub => Sub,
        Mul => Mul,
        CompareTo => CompareTo,
        Equals => Equals,
        And => And,
        Or => Or,
        Xor => Xor,
        Inv => Inv,
        Shl => Shl,
        Shr => Shr,
        Ushr => Ushr,
    }
);

bidirectional_mapping!(DefaultIntegerDivRemV1, crate::IntegerDivRem, {
    Div => Div,
    Rem => Rem,
});

bidirectional_mapping!(
    DefaultPrimitiveBinaryKindV1,
    crate::PrimitiveBinaryKind,
    {
        StringConcat => StringConcat,
        StringCompareTo => StringCompareTo,
    }
);

bidirectional_mapping!(
    DefaultPrimitiveUnaryKindV1,
    crate::PrimitiveUnaryKind,
    { BooleanNot => BooleanNot }
);

bidirectional_mapping!(DefaultArrayAccessKindV1, crate::ArrayAccessKind, {
    ImmutableGet => ImmutableGet,
    MutableGet => MutableGet,
    MutableSet => MutableSet,
});

bidirectional_mapping!(
    DefaultForeignCallbackOperationV1,
    crate::ForeignCallbackOperation,
    {
        Retain => Retain,
        Release => Release,
        State => State,
        Failure => Failure,
    }
);

bidirectional_mapping!(DefaultBinaryOperatorV1, crate::BinOp, {
    Lt => Lt,
    Le => Le,
    Gt => Gt,
    Ge => Ge,
    RefEq => RefEq,
    RefNe => RefNe,
    And => And,
    Or => Or,
});

bidirectional_mapping!(DefaultUnaryOperatorV1, crate::UnOp, { Not => Not });

fn wire_error(decoder: &Decoder<'_, '_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}

#[cfg(test)]
mod tests;
