use crate::modeling::event::Event;
use crate::modeling::hook::Hook;
use crate::modeling::model::Model;
use crate::primitive::time::{Duration, MicroStep, SimTime};
use crate::source_handler::{SourceReadyEntry, SourceView};
use std::io;
use std::io::Write;

pub struct InteractiveStepHook;

impl<E, M: Model<E>> Hook<E, M> for InteractiveStepHook {
    fn before_simulation(&self, _model: &M) {
        // none
    }

    fn after_simulation(&self, _model: &M, _end_tick: SimTime) {
        println!("[Interactive Step Hook] シミュレーションが終了条件に達したため、停止しました。");
    }

    fn before_tick(&self, _model: &M, current_tick: SimTime, skipped_duration: Duration) {
        println!("================ [Interactive Step Hook] ================");
        println!(
            "  これから処理する時刻 (current_tick)   : {:?}",
            current_tick
        );
        println!(
            "  スキップされた時間 (skipped_duration) : {} ticks",
            skipped_duration
        );
        println!("--------------------------------------------------------");

        print!(
            "[Interactive Step Hook] Enterを押すとこのTickの処理（ソース/イベントフェーズ）を開始します... "
        );

        if cfg!(test) || cfg!(feature = "des_sim_test_mode") {
            // テスト中は待機すると止まるので待機処理をスキップする
            return;
        }

        let _ = io::stdout().flush(); // プロンプトを確実に表示させる

        // 2. ユーザーがEnterを押すまでスレッドを完全にブロック（待機）
        let mut input = String::new();
        let _ = io::stdin().read_line(&mut input);
    }

    fn after_tick(&self, _model: &M, current_tick: SimTime, last_micro_step: MicroStep) {
        println!(
            "[Interactive Step Hook] 時刻 {} の処理が完了しました。(総マイクロステップ数: {})",
            current_tick,
            // 最後のマイクロステップの情報が渡ってくるが、0-originのためカウントとするために調整
            last_micro_step.value() + 1
        );
        println!("========================================================\n");
    }

    fn before_micro_step(
        &self,
        _model: &M,
        _current_tick: SimTime,
        _current_micro_step: MicroStep,
    ) {
        // none
    }

    fn after_micro_step(&self, _model: &M, _current_tick: SimTime, _current_micro_step: MicroStep) {
        // none
    }

    fn on_discard_remain_micro_step(
        &self,
        _model: &M,
        _current_tick: SimTime,
        _first_discarded_micro_step: MicroStep,
        _discarded_sources: &[SourceReadyEntry],
        _discarded_events: &[Event<E>],
    ) {
        // none
    }

    fn before_register_source(&self, _model: &M, _name: &str) {
        // none
    }

    fn after_register_source(&self, _model: &M, _name: &str) {
        // none
    }

    fn before_source_phase(
        &self,
        _model: &M,
        _current_tick: SimTime,
        _current_micro_step: MicroStep,
    ) {
        // none
    }

    fn before_source(
        &self,
        _model: &M,
        _current_tick: SimTime,
        _current_micro_step: MicroStep,
        _source_view: &SourceView,
    ) {
        // none
    }

    fn after_source(
        &self,
        _model: &M,
        _current_tick: SimTime,
        _current_micro_step: MicroStep,
        _source_view: &SourceView,
        _computed_next_fire: Option<SimTime>,
    ) {
        // none
    }

    fn cancel_source(
        &self,
        _model: &M,
        _current_tick: SimTime,
        _current_micro_step: MicroStep,
        _scheduled_at: SimTime,
        _source_view: &SourceView,
    ) {
        // none
    }

    fn discard_source(
        &self,
        _model: &M,
        _current_tick: SimTime,
        _current_micro_step: MicroStep,
        _source_view: &SourceView,
    ) {
        // none
    }

    fn after_source_phase(
        &self,
        _model: &M,
        _current_tick: SimTime,
        _current_micro_step: MicroStep,
    ) {
        // none
    }

    fn before_event_phase(
        &self,
        _model: &M,
        _current_tick: SimTime,
        _current_micro_step: MicroStep,
    ) {
        // none
    }

    fn before_event(
        &self,
        _model: &M,
        _current_tick: SimTime,
        _current_micro_step: MicroStep,
        _event: &Event<E>,
    ) {
        // none
    }

    fn after_event(
        &self,
        _model: &M,
        _current_tick: SimTime,
        _current_micro_step: MicroStep,
        _event: &Event<E>,
    ) {
        // none
    }

    fn cancel_event(
        &self,
        _model: &M,
        _current_tick: SimTime,
        _current_micro_step: MicroStep,
        _scheduled_at: SimTime,
        _event: &Event<E>,
    ) {
        // none
    }

    fn discard_event(
        &self,
        _model: &M,
        _current_tick: SimTime,
        _current_micro_step: MicroStep,
        _event: &Event<E>,
    ) {
        // none
    }

    fn after_event_phase(
        &self,
        _model: &M,
        _current_tick: SimTime,
        _current_micro_step: MicroStep,
    ) {
        // none
    }
}
