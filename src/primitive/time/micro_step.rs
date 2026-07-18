use std::fmt::{Display, Formatter};

/// Represents the sequence of micro-steps within a single simulation tick.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct MicroStep(u64);

impl Display for MicroStep {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl MicroStep {
    /// Returns a `MicroStep` initialized to zero.
    #[must_use]
    pub const fn zero() -> Self {
        Self(0)
    }

    /// Creates a new `MicroStep` with the specified value.
    #[cfg(test)]
    #[allow(dead_code)]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the next `MicroStep` in the sequence.
    ///
    /// # Panics
    ///
    /// Panics if incrementing would cause an overflow.
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0 + 1)
    }

    /// Returns the raw numerical value of the micro-step.
    pub const fn value(&self) -> u64 {
        self.0
    }
}

/// Tracks the current state of micro-steps within a simulation tick.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct MicroStepStatus {
    current_micro_step: MicroStep,
}

impl MicroStepStatus {
    /// Creates a new `MicroStepStatus` at the specified micro-step.
    pub(crate) fn new(current_micro_step: MicroStep) -> Self {
        MicroStepStatus { current_micro_step }
    }

    /// Initializes `MicroStepStatus` to the starting step.
    pub(crate) fn initialize() -> Self {
        MicroStepStatus {
            current_micro_step: MicroStep::zero(),
        }
    }

    /// Returns the current `MicroStep`.
    pub fn current(&self) -> MicroStep {
        self.current_micro_step
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn micro_step_zero() {
        let step = MicroStep::zero();
        assert_eq!(step.value(), 0);
    }

    #[test]
    fn micro_step_new_and_value() {
        let step = MicroStep::new(42);
        assert_eq!(step.value(), 42);
    }

    #[test]
    fn micro_step_display() {
        let step = MicroStep::new(100);
        assert_eq!(format!("{}", step), "100");
    }

    #[test]
    fn micro_step_next() {
        let step = MicroStep::zero();
        let next_step = step.next();
        assert_eq!(next_step.value(), 1);

        let step_large = MicroStep::new(99);
        assert_eq!(step_large.next().value(), 100);
    }

    #[test]
    #[should_panic(expected = "attempt to add with overflow")]
    fn micro_step_next_overflow_panic() {
        // Verify that if you call next() in the state of u64::MAX,
        // an overflow panic will occur correctly at runtime (or evaluation time)
        // even if it is within a const fn.
        let max_step = MicroStep::new(u64::MAX);
        let _ = max_step.next();
    }

    #[test]
    fn micro_step_comparison_and_ordering() {
        let step1 = MicroStep::new(10);
        let step2 = MicroStep::new(20);
        let step3 = MicroStep::new(10);

        assert!(step1 < step2);
        assert!(step2 > step1);
        assert!(step1 <= step3);
        assert!(step1 >= step3);
        assert_eq!(step1, step3);
        assert_ne!(step1, step2);
    }

    #[test]
    fn micro_step_status_initialize() {
        let status = MicroStepStatus::initialize();
        assert_eq!(status.current(), MicroStep::zero());
    }

    #[test]
    fn micro_step_status_new_and_current() {
        let step = MicroStep::new(5);
        let status = MicroStepStatus::new(step);
        assert_eq!(status.current(), step);
    }

    #[test]
    fn micro_step_status_mutation_via_recreation() {
        let status_initial = MicroStepStatus::initialize();
        assert_eq!(status_initial.current().value(), 0);

        let next_step = status_initial.current().next();
        let status_next = MicroStepStatus::new(next_step);

        assert_eq!(status_next.current().value(), 1);
        // Ensure the original state remains unchanged.
        assert_eq!(status_initial.current().value(), 0);
    }
}
