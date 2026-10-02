use super::*;

pub(super) enum ResolvedShape {
    Unsigned64 {
        target: Expected,
    },
    Subtractor64 {
        minuend: Expected,
        subtrahend: Expected,
    },
    Branch26 {
        target: Expected,
    },
    Page21 {
        target: Expected,
        explicit_addend: Option<i32>,
    },
    PageOffset12 {
        target: Expected,
        explicit_addend: Option<i32>,
    },
    GotLoadPage21 {
        target: Expected,
    },
    GotLoadPageOffset12 {
        target: Expected,
    },
    PointerToGot32 {
        target: Expected,
    },
    TlvpLoadPage21 {
        target: Expected,
    },
    TlvpLoadPageOffset12 {
        target: Expected,
    },
}

impl ResolvedShape {
    pub(super) fn scoop(
        shape: &Shape,
        target: impl Fn(&Target) -> Result<Expected, LinkError>,
    ) -> Result<Self, LinkError> {
        use Shape as Input;
        Ok(match shape {
            Input::Unsigned64 { target: value } => Self::Unsigned64 {
                target: target(value)?,
            },
            Input::Branch26 { target: value } => Self::Branch26 {
                target: target(value)?,
            },
            Input::GotLoadPage21 { target: value } => Self::GotLoadPage21 {
                target: target(value)?,
            },
            Input::GotLoadPageOffset12 { target: value } => Self::GotLoadPageOffset12 {
                target: target(value)?,
            },
            Input::PointerToGot32 { target: value } => Self::PointerToGot32 {
                target: target(value)?,
            },
            Input::TlvpLoadPage21 { target: value } => Self::TlvpLoadPage21 {
                target: target(value)?,
            },
            Input::TlvpLoadPageOffset12 { target: value } => Self::TlvpLoadPageOffset12 {
                target: target(value)?,
            },
            Input::Page21 {
                target: value,
                explicit_addend,
            } => Self::Page21 {
                target: target(value)?,
                explicit_addend: *explicit_addend,
            },
            Input::PageOffset12 {
                target: value,
                explicit_addend,
            } => Self::PageOffset12 {
                target: target(value)?,
                explicit_addend: *explicit_addend,
            },
            Input::Subtractor64 {
                minuend,
                subtrahend,
            } => Self::Subtractor64 {
                minuend: target(minuend)?,
                subtrahend: target(subtrahend)?,
            },
        })
    }
    pub(super) fn native(
        shape: scoop_slib::DarwinArm64RelocationShapeV1,
        target: impl Fn(scoop_slib::DarwinArm64RelocationTargetV1) -> Result<Expected, LinkError>,
    ) -> Result<Self, LinkError> {
        use scoop_slib::DarwinArm64RelocationShapeV1 as Input;
        Ok(match shape {
            Input::Unsigned64 { target: value } => Self::Unsigned64 {
                target: target(value)?,
            },
            Input::Branch26 { target: value } => Self::Branch26 {
                target: target(value)?,
            },
            Input::GotLoadPage21 { target: value } => Self::GotLoadPage21 {
                target: target(value)?,
            },
            Input::GotLoadPageOffset12 { target: value } => Self::GotLoadPageOffset12 {
                target: target(value)?,
            },
            Input::PointerToGot32 { target: value } => Self::PointerToGot32 {
                target: target(value)?,
            },
            Input::TlvpLoadPage21 { target: value } => Self::TlvpLoadPage21 {
                target: target(value)?,
            },
            Input::TlvpLoadPageOffset12 { target: value } => Self::TlvpLoadPageOffset12 {
                target: target(value)?,
            },
            Input::Page21 {
                target: value,
                explicit_addend,
            } => Self::Page21 {
                target: target(value)?,
                explicit_addend,
            },
            Input::PageOffset12 {
                target: value,
                explicit_addend,
            } => Self::PageOffset12 {
                target: target(value)?,
                explicit_addend,
            },
            Input::Subtractor64 {
                minuend,
                subtrahend,
            } => Self::Subtractor64 {
                minuend: target(minuend)?,
                subtrahend: target(subtrahend)?,
            },
        })
    }
}
