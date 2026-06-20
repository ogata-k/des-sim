#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct MicroStep(u64);

impl MicroStep {
    #[must_use]
    pub const fn zero() -> Self {
        Self(0)
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
