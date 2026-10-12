use scoop_lir::{AbiCarrier, AbiCoercion};
/// SysV assigns all eightbytes of one aggregate or leaves the remaining
/// registers available to later arguments. AArch64 grouped carriers let LLVM
/// apply its consecutive-register and stack rules directly.
pub struct AbiArgumentRegisters {
    gpr: usize,
    sse: usize,
}

impl AbiArgumentRegisters {
    pub fn sysv(indirect_result: bool) -> Self {
        Self {
            gpr: 6 - usize::from(indirect_result),
            sse: 8,
        }
    }
    pub fn scalar(&mut self, carrier: AbiCarrier) {
        match carrier {
            AbiCarrier::Float(_) | AbiCarrier::FloatPair => self.sse = self.sse.saturating_sub(1),
            _ => self.gpr = self.gpr.saturating_sub(1),
        }
    }
    pub fn aggregate(&mut self, coercion: AbiCoercion) -> bool {
        let (mut gpr, mut sse) = (0, 0);
        for part in coercion.parts() {
            match part.carrier() {
                AbiCarrier::Float(_) | AbiCarrier::FloatPair => sse += 1,
                _ => gpr += 1,
            }
        }
        if gpr > self.gpr || sse > self.sse {
            return false;
        }
        self.gpr -= gpr;
        self.sse -= sse;
        true
    }
}
