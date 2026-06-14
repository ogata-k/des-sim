mod context;
mod fired;
mod source;
mod view;

use crate::primitive::id::SourceId;
use crate::primitive::time::{Duration, SimTime};
use crate::world::source::fired::FiredSourceReady;
pub use context::*;
pub use source::*;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::ops::Deref;
use std::sync::Arc;
pub use view::*;

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) struct ScheduledSource {
    scheduled_at: SimTime,
    source_id: SourceId,
}

pub(crate) struct SourceEntry<E> {
    pub(crate) name: Arc<str>,
    pub(crate) source: Box<dyn Source<E>>,
}

pub(crate) struct SourceHandler<E> {
    source_registry: Vec<SourceEntry<E>>,
    next_source_id: usize,
    // Rustは最大ヒープなので、time->source_idの順のソートの小さい順にする
    ready_queue: BinaryHeap<Reverse<ScheduledSource>>,
    pending_queue: BinaryHeap<Reverse<ScheduledSource>>,
}

impl<E> SourceHandler<E> {
    pub fn new() -> SourceHandler<E> {
        SourceHandler {
            source_registry: vec![],
            next_source_id: 0,
            ready_queue: BinaryHeap::new(),
            pending_queue: BinaryHeap::new(),
        }
    }

    /// [Source]を初回起動日時で実行するように登録する。
    /// 使用用途は、初回登録用途。
    pub fn add_source<S>(&mut self, name: String, first_fire_time: SimTime, source: S)
    where
        S: Source<E> + 'static,
    {
        self.source_registry.push(SourceEntry {
            name: Arc::from(name),
            source: Box::new(source),
        });
        self.pending_queue.push(Reverse(ScheduledSource {
            scheduled_at: first_fire_time,
            source_id: SourceId::new(self.next_source_id),
        }));
        self.next_source_id += 1;
    }

    /// [Source]を現在時刻から時間がたった後の時間で実行するように登録する。
    /// 使用用途は、シミュレーション中での使用。
    pub fn add_source_after<S>(&mut self, name: String, now: SimTime, delay: Duration, source: S)
    where
        S: Source<E> + 'static,
    {
        assert!(delay > Duration::zero());

        self.source_registry.push(SourceEntry {
            name: Arc::from(name),
            source: Box::new(source),
        });
        self.pending_queue.push(Reverse(ScheduledSource {
            scheduled_at: now + delay,
            source_id: SourceId::new(self.next_source_id),
        }));
        self.next_source_id += 1;
    }

    /// [Source]を現在時刻の次のマイクロステップで実行するように登録する。
    /// 使用用途は、シミュレーション中での使用。
    pub fn add_source_at_now<S>(&mut self, name: String, now: SimTime, source: S)
    where
        S: Source<E> + 'static,
    {
        self.source_registry.push(SourceEntry {
            name: Arc::from(name),
            source: Box::new(source),
        });
        self.pending_queue.push(Reverse(ScheduledSource {
            scheduled_at: now,
            source_id: SourceId::new(self.next_source_id),
        }));
        self.next_source_id += 1;
    }

    /// now時点で発火しているべきソースをpopしてVecに詰めて返す。
    ///
    /// 実行時に[Duration::zero()]でイベントをスケジュールした場合に、同一時間内でも再度とれる。
    /// なので、同一時間内でさらにループして[self::drain_ready()]した結果が空になるまで取得し続けること。
    /// ※再取得漏れが発生するとpanicするので注意。
    pub fn drain_ready(&mut self, now: SimTime) -> FiredSourceReady {
        let mut fired_source_indexes: Vec<(SourceId, Arc<str>)> = Vec::new();
        while let Some(Reverse(scheduled)) = self.ready_queue.peek() {
            assert!(
                scheduled.scheduled_at >= now,
                "SourceScheduler invariant violated: scheduled_source.scheduled_at={} < now={}",
                scheduled.scheduled_at,
                now
            );
            if scheduled.scheduled_at != now {
                break;
            }

            // 各Sourceで処理されてnowの次のマイクロステップに登録されても処理されないように、先に集めておく。
            let scheduled = self.ready_queue.pop().unwrap().0;
            fired_source_indexes.push((
                scheduled.source_id,
                Arc::clone(&self.source_registry[scheduled.source_id.value()].name),
            ));
        }

        FiredSourceReady::new(fired_source_indexes)
    }

    /// 発火して得られた次のスケジュール時刻でスケジュールしてから、その次のスケジュール時刻を返す。
    ///
    /// [Duration::zero()]で次の発火日時を登録すると、次のマイクロステップで処理されるものとして扱う。
    /// 対して[None]は、これ以降発火することがないソースを表す。
    pub fn fire_and_schedule(
        &mut self,
        now: SimTime,
        context: &mut SourceContext<E>,
        source_id: SourceId,
    ) -> Option<Duration> {
        let entry: &mut SourceEntry<E> = &mut self.source_registry[source_id.value()];
        let next_fire_delay_optional = entry.source.fire(now, context);
        if let Some(next_fire_delay) = next_fire_delay_optional {
            self.pending_queue.push(Reverse(ScheduledSource {
                scheduled_at: now + next_fire_delay,
                source_id,
            }));
        }

        next_fire_delay_optional
    }

    pub fn peek_next_time(&self) -> Option<SimTime> {
        self.ready_queue.peek().map(|i| i.0.scheduled_at)
    }

    #[cfg(test)]
    pub fn peek(&self) -> Option<(SimTime, &ScheduledSource)> {
        self.ready_queue.peek().map(|i| (i.0.scheduled_at, &i.0))
    }

    /// スケジュールされたSourceを反映させる
    pub fn flush_pending(&mut self) {
        self.ready_queue.append(&mut self.pending_queue)
    }
}
