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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sim_time_creation() {
        let t = SimTime::new(10);
        assert_eq!(t.as_tick_value(), 10);
        assert!(!t.is_zero());

        let t_zero = SimTime::zero();
        assert_eq!(t_zero.as_tick_value(), 0);
        assert!(t_zero.is_zero());
    }

    #[test]
    fn sim_time_from_tick() {
        let t = SimTime::from(5);
        assert_eq!(t.as_tick_value(), 5);
    }

    #[test]
    fn sim_time_from_usize_conversion() {
        let val: usize = 123;
        let sim_time = SimTime::from(val);
        assert_eq!(sim_time.as_tick_value(), val);
    }

    #[test]
    fn sim_time_display() {
        let t = SimTime::new(100);
        assert_eq!(format!("{}", t), "100");
    }

    #[test]
    fn sim_time_add_duration() {
        let t1 = SimTime::new(10);
        let d = Duration::ticks(5);
        let t2 = t1 + d;
        assert_eq!(t2.as_tick_value(), 15);
    }

    #[test]
    fn sim_time_add_assign_duration() {
        let mut t = SimTime::new(10);
        let d = Duration::ticks(5);
        t += d;
        assert_eq!(t.as_tick_value(), 15);
    }

    #[test]
    fn sim_time_sub_sim_time() {
        let t1 = SimTime::new(10);
        let t2 = SimTime::new(5);
        let d = t1 - t2;
        assert_eq!(d.as_ticks(), 5);
    }

    #[test]
    fn sim_time_sub_duration() {
        let t = SimTime::new(10);
        let d = Duration::ticks(5);
        let t_new = t - d;
        assert_eq!(t_new.as_tick_value(), 5);
    }

    #[test]
    fn sim_time_checked_add() {
        let t = SimTime::new(10);
        let d = Duration::ticks(5);
        assert_eq!(t.checked_add(d), Some(SimTime::new(15)));
        let max_tick = TimeTick::MAX - 1;
        let t_max = SimTime::new(max_tick);
        let d_one = Duration::one();
        assert_eq!(t_max.checked_add(d_one), Some(SimTime::new(TimeTick::MAX)));
        assert_eq!(t_max.checked_add(Duration::ticks(2)), None);
    }

    #[test]
    fn sim_time_saturating_add() {
        let t = SimTime::new(10);
        let d = Duration::ticks(5);
        assert_eq!(t.saturating_add(d), SimTime::new(15));
        let max_tick = TimeTick::MAX - 1;
        let t_max = SimTime::new(max_tick);
        let d_two = Duration::ticks(2);
        assert_eq!(t_max.saturating_add(d_two), SimTime::new(TimeTick::MAX));
    }

    #[test]
    fn sim_time_checked_sub() {
        let t1 = SimTime::new(10);
        let t2 = SimTime::new(5);
        assert_eq!(t1.checked_sub(t2), Some(Duration::ticks(5)));
        let t_zero = SimTime::zero();
        assert_eq!(t1.checked_sub(SimTime::new(15)), None);
        assert_eq!(t_zero.checked_sub(SimTime::new(1)), None);
    }

    #[test]
    fn sim_time_saturating_sub() {
        let t1 = SimTime::new(10);
        let t2 = SimTime::new(5);
        assert_eq!(t1.saturating_sub(t2), Duration::ticks(5));
        assert_eq!(t1.saturating_sub(SimTime::new(15)), Duration::zero());
    }

    #[test]
    fn sim_time_checked_sub_duration() {
        let t = SimTime::new(10);
        let d = Duration::ticks(5);
        assert_eq!(t.checked_sub_duration(d), Some(SimTime::new(5)));
        assert_eq!(t.checked_sub_duration(Duration::ticks(15)), None);
    }

    #[test]
    fn sim_time_saturating_sub_duration() {
        let t = SimTime::new(10);
        let d = Duration::ticks(5);
        assert_eq!(t.saturating_sub_duration(d), SimTime::new(5));
        assert_eq!(
            t.saturating_sub_duration(Duration::ticks(15)),
            SimTime::zero()
        );
    }

    #[test]
    fn sim_time_add_zero_duration() {
        let t = SimTime::new(100);
        let d_zero = Duration::zero();
        assert_eq!(t + d_zero, t);
    }

    #[test]
    fn sim_time_sub_zero_duration() {
        let t = SimTime::new(100);
        let d_zero = Duration::zero();
        assert_eq!(t - d_zero, t);
    }

    #[test]
    fn sim_time_max_value_subtraction() {
        let max_sim_time = SimTime::new(TimeTick::MAX);
        let one_duration = Duration::one();
        let expected_sim_time = SimTime::new(TimeTick::MAX - 1);
        assert_eq!(max_sim_time - one_duration, expected_sim_time);
    }

    #[test]
    fn sim_time_comparison() {
        let t1 = SimTime::new(10);
        let t2 = SimTime::new(20);
        let t3 = SimTime::new(10);

        assert!(t1 < t2);
        assert!(t2 > t1);
        assert!(t1 <= t3);
        assert!(t1 >= t3);
        assert_eq!(t1, t3);
        assert_ne!(t1, t2);
    }

    #[test]
    fn duration_creation() {
        let d = Duration::ticks(10);
        assert_eq!(d.as_ticks(), 10);
        assert!(!d.is_zero());

        let d_zero = Duration::zero();
        assert_eq!(d_zero.as_ticks(), 0);
        assert!(d_zero.is_zero());

        let d_one = Duration::one();
        assert_eq!(d_one.as_ticks(), 1);
    }

    #[test]
    fn duration_from_tick() {
        let d = Duration::from(5);
        assert_eq!(d.as_ticks(), 5);
    }

    #[test]
    fn duration_from_usize_conversion() {
        let val: usize = 456;
        let duration = Duration::from(val);
        assert_eq!(duration.as_ticks(), val);
    }

    #[test]
    fn duration_display() {
        let d = Duration::ticks(100);
        assert_eq!(format!("{}", d), "100");
    }

    #[test]
    fn duration_add_duration() {
        let d1 = Duration::ticks(10);
        let d2 = Duration::ticks(5);
        let d3 = d1 + d2;
        assert_eq!(d3.as_ticks(), 15);
    }

    #[test]
    fn duration_add_assign_duration() {
        let mut d = Duration::ticks(10);
        let d_add = Duration::ticks(5);
        d += d_add;
        assert_eq!(d.as_ticks(), 15);
    }

    #[test]
    fn duration_sub_duration() {
        let d1 = Duration::ticks(10);
        let d2 = Duration::ticks(5);
        let d3 = d1 - d2;
        assert_eq!(d3.as_ticks(), 5);
    }

    #[test]
    fn duration_sub_assign_duration() {
        let mut d = Duration::ticks(10);
        let d_sub = Duration::ticks(5);
        d -= d_sub;
        assert_eq!(d.as_ticks(), 5);
    }

    #[test]
    fn duration_checked_add() {
        let d1 = Duration::ticks(10);
        let d2 = Duration::ticks(5);
        assert_eq!(d1.checked_add(d2), Some(Duration::ticks(15)));
        let max_tick = TimeTick::MAX - 1;
        let d_max = Duration::ticks(max_tick);
        let d_one = Duration::one();
        assert_eq!(
            d_max.checked_add(d_one),
            Some(Duration::ticks(TimeTick::MAX))
        );
        assert_eq!(d_max.checked_add(Duration::ticks(2)), None);
    }

    #[test]
    fn duration_saturating_add() {
        let d1 = Duration::ticks(10);
        let d2 = Duration::ticks(5);
        assert_eq!(d1.saturating_add(d2), Duration::ticks(15));
        let max_tick = TimeTick::MAX - 1;
        let d_max = Duration::ticks(max_tick);
        let d_two = Duration::ticks(2);
        assert_eq!(d_max.saturating_add(d_two), Duration::ticks(TimeTick::MAX));
    }

    #[test]
    fn duration_checked_sub() {
        let d1 = Duration::ticks(10);
        let d2 = Duration::ticks(5);
        assert_eq!(d1.checked_sub(d2), Some(Duration::ticks(5)));
        assert_eq!(d1.checked_sub(Duration::ticks(15)), None);
    }

    #[test]
    fn duration_saturating_sub() {
        let d1 = Duration::ticks(10);
        let d2 = Duration::ticks(5);
        assert_eq!(d1.saturating_sub(d2), Duration::ticks(5));
        assert_eq!(d1.saturating_sub(Duration::ticks(15)), Duration::zero());
    }

    #[test]
    fn duration_add_zero_duration() {
        let d = Duration::ticks(100);
        let d_zero = Duration::zero();
        assert_eq!(d + d_zero, d);
    }

    #[test]
    fn duration_sub_zero_duration() {
        let d = Duration::ticks(100);
        let d_zero = Duration::zero();
        assert_eq!(d - d_zero, d);
    }

    #[test]
    fn duration_max_value_subtraction() {
        let max_duration = Duration::ticks(TimeTick::MAX);
        let one_duration = Duration::one();
        let expected_duration = Duration::ticks(TimeTick::MAX - 1);
        assert_eq!(max_duration - one_duration, expected_duration);
    }

    #[test]
    fn duration_comparison() {
        let d1 = Duration::ticks(10);
        let d2 = Duration::ticks(20);
        let d3 = Duration::ticks(10);

        assert!(d1 < d2);
        assert!(d2 > d1);
        assert!(d1 <= d3);
        assert!(d1 >= d3);
        assert_eq!(d1, d3);
        assert_ne!(d1, d2);
    }

    #[test]
    fn tick_status_initialize() {
        let status = TickStatus::initialize();
        assert_eq!(status.current(), SimTime::zero());
        assert_eq!(status.skipped(), Duration::zero());
        assert_eq!(status.previous(), SimTime::zero());
    }

    #[test]
    fn tick_status_new() {
        let current = SimTime::new(10);
        let skipped = Duration::ticks(2);
        let status = TickStatus::new(current, skipped);
        assert_eq!(status.current(), current);
        assert_eq!(status.skipped(), skipped);
    }

    #[test]
    fn tick_status_previous() {
        // current_tick = 0, skipped = 0 => previous = 0
        let status = TickStatus::initialize();
        assert_eq!(status.previous(), SimTime::zero());

        // current_tick = 5, skipped = 0 => previous = 5 - 0 - 1 = 4
        let status = TickStatus::new(SimTime::new(5), Duration::zero());
        assert_eq!(status.previous(), SimTime::new(4));

        // current_tick = 5, skipped = 2 => previous = 5 - 2 - 1 = 2
        let status = TickStatus::new(SimTime::new(5), Duration::ticks(2));
        assert_eq!(status.previous(), SimTime::new(2));
    }

    #[test]
    fn tick_status_is_done_ticks_no_skip() {
        // tick_count = 2, include_zero_tick = true
        // previous = 0, 0 + 1 >= 2 => false
        let status = TickStatus::initialize();
        assert!(!status.is_done_ticks(true, 2));

        // previous = 1, 1 + 1 >= 2 => true
        let status = TickStatus::new(SimTime::new(2), Duration::zero());
        assert!(status.is_done_ticks(true, 2));

        // tick_count = 2, include_zero_tick = false
        // previous = 0, 0 + 0 >= 2 => false
        let status = TickStatus::initialize();
        assert!(!status.is_done_ticks(false, 2));

        // previous = 1, 1 + 0 >= 2 => false
        let status = TickStatus::new(SimTime::new(2), Duration::zero());
        assert!(!status.is_done_ticks(false, 2));

        // previous = 2, 2 + 0 >= 2 => true
        let status = TickStatus::new(SimTime::new(3), Duration::zero());
        assert!(status.is_done_ticks(false, 2));
    }

    #[test]
    fn tick_status_is_done_ticks_with_skip() {
        // current_tick = 5, skipped = 2. previous = 2
        // tick_count = 2, include_zero_tick = true
        // previous = 2, 2 + 1 >= 2 => true
        let status = TickStatus::new(SimTime::new(5), Duration::ticks(2));
        assert!(status.is_done_ticks(true, 2));

        // tick_count = 3, include_zero_tick = true
        // previous = 2, 2 + 1 >= 3 => true
        let status = TickStatus::new(SimTime::new(5), Duration::ticks(2));
        assert!(status.is_done_ticks(true, 3));

        // tick_count = 4, include_zero_tick = true
        // previous = 2, 2 + 1 >= 4 => false
        let status = TickStatus::new(SimTime::new(5), Duration::ticks(2));
        assert!(!status.is_done_ticks(true, 4));

        // current_tick = 5, skipped = 2. previous = 2
        // tick_count = 2, include_zero_tick = false
        // previous = 2, 2 + 0 >= 2 => true
        let status = TickStatus::new(SimTime::new(5), Duration::ticks(2));
        assert!(status.is_done_ticks(false, 2));

        // tick_count = 3, include_zero_tick = false
        // previous = 2, 2 + 0 >= 3 => false
        let status = TickStatus::new(SimTime::new(5), Duration::ticks(2));
        assert!(!status.is_done_ticks(false, 3));
    }

    #[test]
    fn tick_status_previous_large_current_tick() {
        let current = SimTime::new(TimeTick::MAX);
        let skipped = Duration::ticks(100);
        let status = TickStatus::new(current, skipped);
        assert_eq!(status.previous(), SimTime::new(TimeTick::MAX - 100 - 1));
    }

    #[test]
    fn tick_status_is_done_ticks_edge_cases() {
        // tick_count = 0
        let status = TickStatus::initialize();
        assert!(status.is_done_ticks(true, 0));
        assert!(status.is_done_ticks(false, 0));

        // tick_count = 1, include_zero_tick = true
        // previous = 0, 0 + 1 >= 1 => true
        let status = TickStatus::initialize();
        assert!(status.is_done_ticks(true, 1));

        // tick_count = 1, include_zero_tick = false
        // previous = 0, 0 + 0 >= 1 => false
        let status = TickStatus::initialize();
        assert!(!status.is_done_ticks(false, 1));

        // current_tick = 1, skipped = 0. previous = 0
        // tick_count = 1, include_zero_tick = false
        // previous = 0, 0 + 0 >= 1 => false
        let status = TickStatus::new(SimTime::new(1), Duration::zero());
        assert!(!status.is_done_ticks(false, 1));

        // current_tick = 2, skipped = 0. previous = 1
        // tick_count = 1, include_zero_tick = false
        // previous = 1, 1 + 0 >= 1 => true
        let status = TickStatus::new(SimTime::new(2), Duration::zero());
        assert!(status.is_done_ticks(false, 1));
    }

    #[test]
    fn tick_status_previous_with_skipped_equal_to_current_minus_one() {
        // current_tick = 5, skipped = 4 => previous = 5 - 4 - 1 = 0
        let status = TickStatus::new(SimTime::new(5), Duration::ticks(4));
        assert_eq!(status.previous(), SimTime::zero());
    }

    #[test]
    fn tick_status_previous_with_skipped_greater_than_current_minus_one() {
        // current_tick = 5, skipped = 5 => previous = 5 - 5 - 1 = 0 (saturating sub)
        let status = TickStatus::new(SimTime::new(5), Duration::ticks(5));
        assert_eq!(status.previous(), SimTime::zero());
    }
}
