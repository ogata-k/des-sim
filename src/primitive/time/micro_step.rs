use std::fmt::{Display, Formatter};

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct MicroStep(u64);

impl Display for MicroStep {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl MicroStep {
    #[must_use]
    pub const fn zero() -> Self {
        Self(0)
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0 + 1)
    }

    pub const fn value(&self) -> u64 {
        self.0
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct MicroStepStatus {
    current_micro_step: MicroStep,
}

impl MicroStepStatus {
    pub(crate) fn new(current_micro_step: MicroStep) -> Self {
        MicroStepStatus { current_micro_step }
    }

    pub(crate) fn initialize() -> Self {
        MicroStepStatus {
            current_micro_step: MicroStep::zero(),
        }
    }

    pub fn current(&self) -> MicroStep {
        self.current_micro_step
    }
}
