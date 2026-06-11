use std::fmt::{Display, Formatter};
use std::ops::{Add, AddAssign, Sub, SubAssign};

pub type TimeTick = u64;

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct SimTime(TimeTick);

impl Display for SimTime {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<TimeTick> for SimTime {
    fn from(v: TimeTick) -> Self {
        SimTime(v)
    }
}

impl Default for SimTime {
    fn default() -> Self {
        SimTime::zero()
    }
}

impl Add<Duration> for SimTime {
    type Output = SimTime;

    fn add(self, rhs: Duration) -> Self::Output {
        SimTime(self.0 + rhs.0)
    }
}

impl AddAssign<Duration> for SimTime {
    fn add_assign(&mut self, rhs: Duration) {
        self.0 += rhs.0
    }
}

impl Sub<SimTime> for SimTime {
    type Output = Duration;

    fn sub(self, rhs: SimTime) -> Self::Output {
        debug_assert!(self.0 >= rhs.0);
        Duration(self.0 - rhs.0)
    }
}

impl Sub<Duration> for SimTime {
    type Output = SimTime;

    fn sub(self, rhs: Duration) -> Self::Output {
        debug_assert!(self.0 >= rhs.0);
        SimTime(self.0 - rhs.0)
    }
}

impl SimTime {
    pub const fn new(ticks: TimeTick) -> SimTime {
        SimTime(ticks)
    }

    pub const fn zero() -> SimTime {
        SimTime(0)
    }

    pub const fn as_ticks(self) -> TimeTick {
        self.0
    }

    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    pub fn checked_add(self, rhs: Duration) -> Option<SimTime> {
        self.0.checked_add(rhs.0).map(SimTime)
    }

    pub fn saturating_add(self, rhs: Duration) -> SimTime {
        SimTime(self.0.saturating_add(rhs.0))
    }

    pub fn checked_sub(self, rhs: SimTime) -> Option<Duration> {
        self.0.checked_sub(rhs.0).map(Duration)
    }

    pub fn saturating_sub(self, rhs: SimTime) -> Duration {
        Duration(self.0.saturating_sub(rhs.0))
    }

    pub fn checked_sub_duration(self, rhs: Duration) -> Option<SimTime> {
        self.0.checked_sub(rhs.0).map(SimTime)
    }

    pub fn saturating_sub_duration(self, rhs: Duration) -> SimTime {
        SimTime(self.0.saturating_sub(rhs.0))
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct Duration(TimeTick);

impl Display for Duration {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<TimeTick> for Duration {
    fn from(v: TimeTick) -> Self {
        Duration(v)
    }
}

impl Default for Duration {
    fn default() -> Self {
        Duration::zero()
    }
}

impl Add<Duration> for Duration {
    type Output = Duration;

    fn add(self, rhs: Duration) -> Self::Output {
        Duration(self.0 + rhs.0)
    }
}

impl AddAssign<Duration> for Duration {
    fn add_assign(&mut self, rhs: Duration) {
        self.0 += rhs.0;
    }
}

impl Sub<Duration> for Duration {
    type Output = Duration;

    fn sub(self, rhs: Duration) -> Self::Output {
        debug_assert!(self.0 >= rhs.0);
        Duration(self.0 - rhs.0)
    }
}

impl SubAssign<Duration> for Duration {
    fn sub_assign(&mut self, rhs: Duration) {
        debug_assert!(self.0 >= rhs.0);
        self.0 -= rhs.0;
    }
}

impl Duration {
    pub const fn ticks(ticks: TimeTick) -> Duration {
        Duration(ticks)
    }

    pub const fn zero() -> Duration {
        Duration(0)
    }

    pub const fn one() -> Duration {
        Duration(1)
    }

    pub const fn as_ticks(self) -> TimeTick {
        self.0
    }

    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    pub fn checked_add(self, rhs: Duration) -> Option<Duration> {
        self.0.checked_add(rhs.0).map(Duration)
    }

    pub fn saturating_add(self, rhs: Duration) -> Duration {
        Duration(self.0.saturating_add(rhs.0))
    }

    pub fn checked_sub(self, rhs: Duration) -> Option<Duration> {
        self.0.checked_sub(rhs.0).map(Duration)
    }

    pub fn saturating_sub(self, rhs: Duration) -> Duration {
        Duration(self.0.saturating_sub(rhs.0))
    }
}
