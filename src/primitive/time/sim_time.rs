use std::fmt::{Display, Formatter};
use std::ops::{Add, AddAssign, Sub, SubAssign};

pub type TimeTick = usize;

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

    pub const fn as_tick_value(self) -> TimeTick {
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

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct TickStatus {
    current_tick: SimTime,
    skipped: Duration,
}
impl TickStatus {
    pub(crate) fn new(current_tick: SimTime, skipped: Duration) -> Self {
        TickStatus {
            current_tick,
            skipped,
        }
    }

    pub(crate) fn initialize() -> Self {
        TickStatus {
            current_tick: SimTime::zero(),
            skipped: Duration::zero(),
        }
    }

    pub fn current(&self) -> SimTime {
        self.current_tick
    }

    pub fn skipped(&self) -> Duration {
        self.skipped
    }

    pub fn previous(&self) -> SimTime {
        if self.current_tick == SimTime::zero() {
            self.current_tick
        } else {
            self.current_tick - self.skipped - Duration::one()
        }
    }

    /// 時間スキップを考慮し「実際に処理を終えている時間（`previous()`）」をベースに判定。
    /// 連続してTickが進む場合（例: tick_count=2 のとき）：
    ///   0 tick開始時: previous=0 (0+1>=2 => false) -> 0 tick処理実行
    ///   1 tick開始時: previous=0 (0+1>=2 => false) -> 1 tick処理実行
    ///   2 tick開始時: previous=1 (1+1>=2 => true)  -> ここで処理開始前に停止！
    ///
    /// 時間が大きくスキップする場合（例: tick_count=2 で 0 tick → 5 tick へジャンプ）：
    ///   0 tick開始時: previous=0 (0+1>=2 => false) -> 0 tick処理実行（ここで5へジャンプ）
    ///   5 tick開始時: previous=0 (0+1>=2 => false) -> 5 tick処理を実行（ジャンプ直後の実処理）
    ///   6 tick開始時: previous=5 (5+1>=2 => true)  -> ここで停止！
    ///
    /// これにより、スキップが発生しても「最低限、閾値をまたぐ直前の処理（5 tick）」までは
    /// 確実に実行を完了させてから安全にシミュレーションを止めることができます。
    pub fn is_done_ticks(&self, include_zero_tick: bool, tick_count: TimeTick) -> bool {
        self.previous().as_tick_value() + if include_zero_tick { 1 } else { 0 } >= tick_count
    }
}
