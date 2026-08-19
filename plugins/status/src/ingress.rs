//! 파이프 수신구 — 모든 감지 수단이 여기 한 곳으로 들어온다.
//!
//! opencode 는 SSE, claude 는 훅, codex 는 notify 로 신호가 오지만 **경로는 하나다**:
//! 외부의 얇은 브리지가 원문을 `zellij pipe` 의 STDIN 으로 흘려보내고, 해석은 전부 여기서 한다.
//!
//! 왜 플러그인 안에서 직접 SSE 를 읽지 않는가 — 읽을 수 없다. zellij 0.44.3 의 `web_request` 는
//! 단발성(`WebRequestResult` 로 본문 전체 1회)이고 `run_command` 는 프로세스가 끝나야 출력을 준다.
//! 무한 스트림에 둘 다 맞지 않는다. 반면 `zellij pipe` 는 STDIN 스트리밍을 지원한다.
//! 근거: `zellij-tile-0.44.3/src/shim.rs:828,862`, `zellij pipe --help` 의 `tail -f | zellij pipe` 예시.

use std::collections::BTreeMap;

use polycanv_protocol::event::StatusRecord;
use polycanv_protocol::{AgentState, PaneKey, StatusEvent, StatusSource};

use crate::adapters::{self, StatusAdapter};

/// 브리지가 쓰는 파이프 이름. `zellij pipe --name` 값과 반드시 같아야 한다.
pub const PIPE_NAME: &str = "polycanv:status";
/// 사이드바로 상태 변화를 흘려보낼 때 쓰는 이름.
pub const BROADCAST_NAME: &str = "polycanv:state";

/// 파이프 args 키.
pub const ARG_TOOL: &str = "tool";
pub const ARG_PANE: &str = "pane_id";

#[derive(Default)]
pub struct Ingress {
    /// 패인마다 어댑터를 따로 둔다 — 어댑터는 직전 맥락을 들고 있으므로 공유하면 섞인다.
    adapters: BTreeMap<PaneKey, Box<dyn StatusAdapter>>,
    records: BTreeMap<PaneKey, StatusRecord>,
}

impl Ingress {
    pub fn new() -> Self {
        Self::default()
    }

    /// 원문 이벤트 한 줄을 넣는다. **상태가 실제로 바뀌었을 때만** 이벤트를 돌려준다.
    ///
    /// 바뀌지 않은 이벤트까지 사이드바로 흘려보내면 opencode 의 `message.part.updated` 같은
    /// 초당 수십 건짜리 스트림이 그대로 렌더 폭풍이 된다.
    pub fn ingest(
        &mut self,
        tool: &str,
        pane: PaneKey,
        raw: &str,
        at_ms: u64,
    ) -> Option<StatusEvent> {
        let adapter = match self.adapters.get_mut(&pane) {
            Some(a) => a,
            None => self
                .adapters
                .entry(pane)
                .or_insert(adapters::for_tool(tool)?),
        };
        let state = adapter.observe(raw)?;
        let ev = StatusEvent {
            pane,
            state,
            source: adapter.source(),
            at_ms,
        };

        // 처음 보는 패인은 "가장 약한 근거의 ⚪"에서 시작한다. 그래야 첫 이벤트가 무조건 채택된다.
        let rec = self
            .records
            .entry(pane)
            .or_insert_with(|| StatusRecord::new(AgentState::Idle, StatusSource::IdleHeuristic, 0));
        let before = rec.state;
        // `apply` 는 "채택했는가"를 돌려주지 `바뀌었는가`를 돌려주지 않는다. 같은 상태를 재채택하는
        // 것은 정상이지만(출처·시각 갱신) 그걸 밖으로 흘리면 안 된다.
        rec.apply(&ev);
        (rec.state != before).then_some(ev)
    }

    /// 사용자가 이 패인을 포커스했다. 🔴이 해제됐으면 `true`.
    pub fn focus(&mut self, pane: PaneKey, at_ms: u64) -> bool {
        match self.records.get_mut(&pane) {
            Some(rec) => {
                let before = rec.state;
                rec.acknowledge(at_ms);
                before != rec.state
            }
            None => false,
        }
    }

    /// 패인이 닫혔다. 어댑터까지 같이 버린다 — 패인 id 는 재사용될 수 있고,
    /// 남은 어댑터의 직전 맥락이 새 패인으로 새어 들어가면 엉뚱한 🔴이 뜬다.
    pub fn forget(&mut self, pane: PaneKey) {
        self.adapters.remove(&pane);
        self.records.remove(&pane);
    }

    pub fn state(&self, pane: PaneKey) -> AgentState {
        self.records
            .get(&pane)
            .map_or(AgentState::Idle, |r| r.state)
    }

    pub fn snapshot(&self) -> impl Iterator<Item = (PaneKey, AgentState)> + '_ {
        self.records.iter().map(|(k, r)| (*k, r.state))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const P1: PaneKey = PaneKey::Terminal(1);
    const P2: PaneKey = PaneKey::Terminal(2);

    fn busy(sid: &str) -> String {
        format!(
            r#"{{"type":"session.status","properties":{{"sessionID":"{sid}","status":{{"type":"busy"}}}}}}"#
        )
    }

    fn idle_ev(sid: &str) -> String {
        format!(r#"{{"type":"session.idle","properties":{{"sessionID":"{sid}"}}}}"#)
    }

    #[test]
    fn 상태가_바뀔_때만_이벤트가_나온다() {
        let mut ing = Ingress::new();
        assert!(ing.ingest("opencode", P1, &busy("s"), 100).is_some());
        assert!(
            ing.ingest("opencode", P1, &busy("s"), 200).is_none(),
            "같은 🟢을 다시 흘리지 않는다"
        );
        assert_eq!(ing.state(P1), AgentState::Running);
    }

    #[test]
    fn 패인마다_맥락이_섞이지_않는다() {
        let mut ing = Ingress::new();
        ing.ingest("opencode", P1, &busy("s"), 100);
        // P2 는 일한 적이 없다. P1 의 busy 가 새면 여기서 🔴이 뜬다.
        assert_eq!(
            ing.ingest("opencode", P2, &idle_ev("s"), 200)
                .map(|e| e.state),
            None,
            "일한 적 없는 패인의 idle 은 ⚪이고, 처음부터 ⚪였으니 변화가 없다"
        );
        assert_eq!(ing.state(P2), AgentState::Idle);
        assert_eq!(ing.state(P1), AgentState::Running);
    }

    #[test]
    fn 어댑터가_없는_도구는_조용히_넘어간다() {
        // codex 는 아직 ①계층 어댑터가 없다. 여기서 억지로 상태를 만들면 오탐이 된다.
        let mut ing = Ingress::new();
        assert!(ing.ingest("codex", P1, &busy("s"), 100).is_none());
        assert_eq!(ing.state(P1), AgentState::Idle);
    }

    #[test]
    fn 포커스하면_완료가_해제된다() {
        let mut ing = Ingress::new();
        ing.ingest("opencode", P1, &busy("s"), 100);
        ing.ingest("opencode", P1, &idle_ev("s"), 200);
        assert_eq!(ing.state(P1), AgentState::Finished);
        assert!(ing.focus(P1, 300));
        assert_eq!(ing.state(P1), AgentState::Idle);
        assert!(!ing.focus(P1, 400), "이미 해제된 뒤의 포커스는 변화가 없다");
    }

    #[test]
    fn 닫힌_패인의_맥락은_남지_않는다() {
        let mut ing = Ingress::new();
        ing.ingest("opencode", P1, &busy("s"), 100);
        ing.forget(P1);
        // 패인 번호가 재사용됐다. 이전 busy 가 남아 있으면 여기서 🔴이 뜬다.
        assert_eq!(
            ing.ingest("opencode", P1, &idle_ev("s"), 200)
                .map(|e| e.state),
            None
        );
        assert_eq!(ing.state(P1), AgentState::Idle);
    }
}
