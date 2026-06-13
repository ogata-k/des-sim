mod context;
mod source;

use crate::primitive::time::{Duration, SimTime};
pub use context::*;
pub use source::*;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) struct ScheduledSource {
    scheduled_at: SimTime,
    source_index: usize,
}

pub(crate) struct SourceEntry<E> {
    pub(crate) name: String,
    pub(crate) source: Box<dyn Source<E>>,
}

pub(crate) struct SourceHandler<E> {
    source_registry: Vec<SourceEntry<E>>,
    source_indexer: usize,
    // Rustは最大ヒープなので、time->source_idの順のソートの小さい順にする
    ready_queue: BinaryHeap<Reverse<ScheduledSource>>,
    pending_queue: BinaryHeap<Reverse<ScheduledSource>>,
}

impl<E> SourceHandler<E> {
    pub fn new() -> SourceHandler<E> {
        SourceHandler {
            source_registry: vec![],
            source_indexer: 0,
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
            name,
            source: Box::new(source),
        });
        self.pending_queue.push(Reverse(ScheduledSource {
            scheduled_at: first_fire_time,
            source_index: self.source_indexer,
        }));
        self.source_indexer += 1;
    }

    /// [Source]を現在時刻から時間がたった後の時間で実行するように登録する。
    /// 使用用途は、シミュレーション中での使用。
    pub fn add_source_after<S>(&mut self, name: String, now: SimTime, delay: Duration, source: S)
    where
        S: Source<E> + 'static,
    {
        assert!(delay > Duration::zero());

        self.source_registry.push(SourceEntry {
            name,
            source: Box::new(source),
        });
        self.pending_queue.push(Reverse(ScheduledSource {
            scheduled_at: now + delay,
            source_index: self.source_indexer,
        }));
        self.source_indexer += 1;
    }

    /// [Source]を現在時刻の次のマイクロステップで実行するように登録する。
    /// 使用用途は、シミュレーション中での使用。
    pub fn add_source_at_now<S>(&mut self, name: String, now: SimTime, source: S)
    where
        S: Source<E> + 'static,
    {
        self.source_registry.push(SourceEntry {
            name,
            source: Box::new(source),
        });
        self.pending_queue.push(Reverse(ScheduledSource {
            scheduled_at: now,
            source_index: self.source_indexer,
        }));
        self.source_indexer += 1;
    }

    /// now時点で発火しているべきソースを処理して、次のソース発火日時を登録する。
    ///
    /// [Duration::zero()]で次の発火日時を登録すると、次のマイクロステップで処理されるものとして扱われるため、
    /// 次のマイクロステップで処理をしてあげないとpanicするので注意。
    pub fn run_ready(&mut self, now: SimTime, context: &mut SourceContext<E>) {
        let mut fired_source_indexes: Vec<usize> = Vec::new();
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
            fired_source_indexes.push(self.ready_queue.pop().unwrap().0.source_index);
        }

        for source_index in fired_source_indexes {
            let entry: &mut SourceEntry<E> = &mut self.source_registry[source_index];
            let next_fire_delay_optional = entry.source.fire(now, context);
            if let Some(next_fire_delay) = next_fire_delay_optional {
                self.pending_queue.push(Reverse(ScheduledSource {
                    scheduled_at: now + next_fire_delay,
                    source_index,
                }));
            }
        }
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
