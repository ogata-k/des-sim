mod fired;
mod view;

pub use fired::*;
pub use view::*;

use crate::modeling::model::Model;
use crate::modeling::source::Source;
use crate::primitive::id::SourceId;
use crate::primitive::time::{Duration, SimTime};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};
use std::sync::Arc;

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) struct ScheduledSource {
    pub(crate) scheduled_at: SimTime,
    pub(crate) source_id: SourceId,
}

pub(crate) struct SourceEntry<E, M: Model<E>> {
    pub(crate) name: Arc<str>,
    pub(crate) source: Box<dyn Source<E, M>>,
}

pub(crate) struct SourceHandler<E, M: Model<E>> {
    source_registry: Vec<SourceEntry<E, M>>,
    next_source_id: usize,
    // Rustは最大ヒープなので、time->source_idの順のソートの小さい順にする
    ready_queue: BinaryHeap<Reverse<ScheduledSource>>,
    pending_queue: BinaryHeap<Reverse<ScheduledSource>>,
}

impl<E, M: Model<E>> SourceHandler<E, M> {
    pub fn new() -> SourceHandler<E, M> {
        SourceHandler {
            source_registry: vec![],
            next_source_id: 0,
            ready_queue: BinaryHeap::new(),
            pending_queue: BinaryHeap::new(),
        }
    }

    pub(crate) fn initialize_sources<F>(&mut self, mut initializer: F)
    where
        F: FnMut(&mut SourceEntry<E, M>),
    {
        self.source_registry.iter_mut().for_each(|e| {
            initializer(e);
        })
    }

    /// [Source]を初回起動日時で実行するように登録する。
    /// 使用用途は、初回登録用途。
    pub fn add_source<S>(&mut self, name: &'static str, first_fire_time: SimTime, source: S)
    where
        S: Source<E, M> + 'static,
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
    pub fn add_source_after<S>(
        &mut self,
        name: &'static str,
        current_tick: SimTime,
        delay: Duration,
        source: S,
    ) where
        S: Source<E, M> + 'static,
    {
        assert!(delay > Duration::zero());

        self.source_registry.push(SourceEntry {
            name: Arc::from(name),
            source: Box::new(source),
        });
        self.pending_queue.push(Reverse(ScheduledSource {
            scheduled_at: current_tick + delay,
            source_id: SourceId::new(self.next_source_id),
        }));
        self.next_source_id += 1;
    }

    /// [Source]を現在時刻の次のマイクロステップで実行するように登録する。
    /// 使用用途は、シミュレーション中での使用。
    pub fn add_source_at_now<S>(&mut self, name: &'static str, current_tick: SimTime, source: S)
    where
        S: Source<E, M> + 'static,
    {
        self.source_registry.push(SourceEntry {
            name: Arc::from(name),
            source: Box::new(source),
        });
        self.pending_queue.push(Reverse(ScheduledSource {
            scheduled_at: current_tick,
            source_id: SourceId::new(self.next_source_id),
        }));
        self.next_source_id += 1;
    }

    /// now時点で発火しているべきソースをpopしてVecに詰めて返す。
    ///
    /// 実行時に[Duration::zero()]でイベントをスケジュールした場合に、同一時間内でも再度とれる。
    /// なので、同一時間内でさらにループして[self::drain_ready()]した結果が空になるまで取得し続けること。
    /// ※再取得漏れが発生するとpanicするので注意。
    pub fn drain_ready(&mut self, current_tick: SimTime) -> VecDeque<SourceReadyEntry> {
        let mut fired_source_indexes: VecDeque<SourceReadyEntry> = VecDeque::new();
        while let Some(Reverse(scheduled)) = self.ready_queue.peek() {
            assert!(
                scheduled.scheduled_at >= current_tick,
                "SourceScheduler invariant violated: scheduled_source.scheduled_at={} < now={}",
                scheduled.scheduled_at,
                current_tick
            );
            if scheduled.scheduled_at != current_tick {
                break;
            }

            // 各Sourceで処理されてnowの次のマイクロステップに登録されても処理されないように、先に集めておく。
            let scheduled = self.ready_queue.pop().unwrap().0;
            fired_source_indexes.push_back(SourceReadyEntry::new(
                scheduled.source_id,
                Arc::clone(&self.source_registry[scheduled.source_id.value()].name),
            ));
        }

        fired_source_indexes
    }

    pub(crate) fn get_by_source_id(&mut self, source_id: SourceId) -> &mut SourceEntry<E, M> {
        &mut self.source_registry[source_id.value()]
    }

    pub(crate) fn schedule_next(
        &mut self,
        current_tick: SimTime,
        next_fire_delay: Duration,
        source_id: SourceId,
    ) {
        self.pending_queue.push(Reverse(ScheduledSource {
            scheduled_at: current_tick + next_fire_delay,
            source_id,
        }));
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
