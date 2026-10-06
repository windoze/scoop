use super::*;
use scoop_identity::FloatConversion;

impl Writer<'_, '_> {
    pub(super) fn float_conversion(&mut self, conversion: crate::LirFloatConversion) -> Result {
        match conversion {
            FloatConversion::FromInteger { source, target } => {
                record!(self, 1; self.integer_kind(source), self.u(u64::from(target.bits())))
            }
            FloatConversion::ToInteger { source, target } => {
                record!(self, 2; self.u(u64::from(source.bits())), self.integer_kind(target))
            }
            FloatConversion::BetweenFloats { source, target } => {
                record!(self, 3; self.u(u64::from(source.bits())), self.u(u64::from(target.bits())))
            }
        }
    }
}
