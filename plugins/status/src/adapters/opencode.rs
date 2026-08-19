//! opencode 어댑터 — HTTP SSE `GET /event` 스트림을 신호등으로 옮긴다.
//!
//! 4개 CLI 중 상태 모델이 신호등과 가장 정확히 맞는다. running/waiting/finished 가
//! 서로 다른 이벤트로 깔끔히 갈리므로 **출력 패턴 매칭을 섞지 않는다.**
//!
//! ⚠️ **구독할 엔드포인트는 `/global/event` 다. `/event` 가 아니다.**
//! opencode 1.14.48 실측: `/event` 와 `/event?directory=<cwd>` 는 `server.connected` 하나만 주고
//! 세션 이벤트가 전혀 오지 않는다. 같은 프롬프트를 도는 동안 `/global/event` 는
//! `session.status{busy}` → `session.status{idle}` → `session.idle` 을 모두 보냈다.
//! (`docs/research/cli-status-hooks.md` §3 의 `GET /event` 는 OpenAPI 스펙만 보고 쓴 것이라 틀렸다.)
//!
//! ⚠️ **승인 이벤트 이름도 SDK 타입과 다르다.** 실측은 `permission.asked`(`properties.id`) /
//! `permission.replied`(`properties.requestID`) 이고, SDK 타입의 `permission.updated` /
//! `permissionID` 는 이 서버에서 발화하지 않았다. 양쪽 다 받도록 해 뒀다.
//!
//! 실측 원문은 `tests/fixtures/` 에 그대로 두었다.

use std::collections::BTreeMap;

use polycanv_protocol::{AgentState, StatusSource};
use serde_json::Value;

use super::StatusAdapter;

#[derive(Debug, Default)]
pub struct OpenCodeAdapter {
    /// "이 세션이 마지막 idle 이후 실제로 일을 했는가." `session.idle` 하나로는
    /// 🔴finished 인지 ⚪idle 인지 알 수 없다 — 그 차이는 직전 `busy` 를 봤는지에만 있다.
    worked: BTreeMap<String, ()>,
    /// 아직 응답되지 않은 승인 요청. permissionID → sessionID.
    pending: BTreeMap<String, String>,
}

impl OpenCodeAdapter {
    /// 세션이 멈췄다. **일하다 멈춘 것일 때만** 🔴을 낸다.
    ///
    /// 이미 멈춰 있던 세션의 idle 은 아무 정보도 없으므로 `None` 이다. ⚪를 돌려주면 안 된다 —
    /// opencode 는 한 턴이 끝날 때 `session.status{idle}` 과 `session.idle` 을 **연달아 둘 다** 보내고
    /// (실측: `tests/fixtures/opencode-global-event.log`), 두 번째가 ⚪를 내면 방금 켠 🔴이
    /// 곧바로 꺼져 사용자가 완료를 놓친다. 🔴은 사용자가 포커스할 때만 해제된다.
    fn settle(&mut self, session: &str) -> Option<AgentState> {
        self.pending.retain(|_, sid| sid != session);
        self.worked.remove(session).map(|_| AgentState::Finished)
    }

    /// 일하는 중이라고 보고한다. 단 **승인 대기가 걸려 있으면 상태를 내리지 않는다.**
    ///
    /// opencode 는 승인 프롬프트가 떠 있는 동안에도 세션을 busy 로 보고할 수 있다.
    /// 그때 🟢로 덮어쓰면 사용자가 답해줘야 할 프롬프트를 놓친다 — 미탐이 오탐보다 나쁘다.
    fn work(&mut self, session: &str) -> Option<AgentState> {
        self.worked.insert(session.to_string(), ());
        if self.pending.is_empty() {
            Some(AgentState::Running)
        } else {
            None
        }
    }
}

impl StatusAdapter for OpenCodeAdapter {
    fn observe(&mut self, raw: &str) -> Option<AgentState> {
        let json = strip_sse(raw)?;
        let root: Value = serde_json::from_str(json).ok()?;
        // `/global/event` 는 이벤트를 `{directory, project, payload:{...}}` 로 감싸서 준다.
        // `/event` 는 감싸지 않는다. 둘 다 받는다 — 감싼 쪽만 처리하면 엔드포인트가 바뀔 때 조용히 먹통이 된다.
        let ev = root
            .get("payload")
            .filter(|p| p.is_object())
            .unwrap_or(&root);
        let props = ev.get("properties");
        let session = props
            .and_then(|p| p.get("sessionID"))
            .and_then(Value::as_str)
            .unwrap_or("");

        match ev.get("type").and_then(Value::as_str)? {
            "session.status" => {
                match props
                    .and_then(|p| p.get("status"))
                    .and_then(|s| s.get("type"))
                    .and_then(Value::as_str)?
                {
                    // retry 는 자동 재시도 중이다 — 사용자가 할 일이 없으므로 여전히 🟢.
                    "busy" | "retry" => self.work(session),
                    "idle" => self.settle(session),
                    _ => None,
                }
            }
            "session.idle" => self.settle(session),
            // 실측 이름은 `permission.asked` 다. `permission.updated` 는 SDK 타입에만 있고
            // 1.14.48 서버는 보내지 않는다 — 버전 차이에 대비해 둘 다 받는다.
            "permission.asked" | "permission.updated" => {
                let id = props.and_then(|p| p.get("id")).and_then(Value::as_str)?;
                self.pending.insert(id.to_string(), session.to_string());
                Some(AgentState::Waiting)
            }
            "permission.replied" => {
                // 실측 필드는 `requestID` 다. SDK 타입의 `permissionID` 도 함께 받는다.
                let id = props
                    .and_then(|p| p.get("requestID").or_else(|| p.get("permissionID")))
                    .and_then(Value::as_str)?;
                self.pending.remove(id);
                // 답을 했으니 opencode 는 작업을 재개한다. 남은 승인이 있으면 아직 🟡이다.
                if self.pending.is_empty() {
                    Some(AgentState::Running)
                } else {
                    None
                }
            }
            // 에러도 사용자가 봐야 한다. 조용히 ⚪로 두면 실패를 놓친다.
            "session.error" => {
                self.pending.retain(|_, sid| sid != session);
                self.worked.remove(session);
                Some(AgentState::Finished)
            }
            _ => None,
        }
    }

    fn source(&self) -> StatusSource {
        StatusSource::Sse
    }
}

/// SSE 한 줄에서 JSON 본문만 꺼낸다.
///
/// 브리지가 원문을 그대로 흘려보내므로 `data: ` 접두사, 하트비트 주석(`:`), 빈 줄이 섞여 들어온다.
/// 접두사가 없는 순수 JSON 도 받는다 — 훅처럼 JSON 을 바로 던지는 경로와 형식을 맞추기 위해서다.
fn strip_sse(raw: &str) -> Option<&str> {
    let line = raw.trim();
    if line.is_empty() || line.starts_with(':') {
        return None;
    }
    let body = line.strip_prefix("data:").unwrap_or(line).trim_start();
    body.starts_with('{').then_some(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn busy(sid: &str) -> String {
        format!(
            r#"data: {{"id":"evt_1","type":"session.status","properties":{{"sessionID":"{sid}","status":{{"type":"busy"}}}}}}"#
        )
    }

    fn idle_ev(sid: &str) -> String {
        format!(
            r#"data: {{"id":"evt_2","type":"session.idle","properties":{{"sessionID":"{sid}"}}}}"#
        )
    }

    /// 실측 형태 그대로 (`permission.asked`, `properties.id`).
    fn ask(id: &str, sid: &str) -> String {
        format!(
            r#"data: {{"id":"evt_3","type":"permission.asked","properties":{{"id":"{id}","sessionID":"{sid}","permission":"bash","patterns":["echo hi"],"always":["echo *"],"metadata":{{}}}}}}"#
        )
    }

    /// 실측 형태 그대로 (`properties.requestID`, `reply`).
    fn replied(id: &str, sid: &str) -> String {
        format!(
            r#"data: {{"id":"evt_4","type":"permission.replied","properties":{{"sessionID":"{sid}","requestID":"{id}","reply":"once"}}}}"#
        )
    }

    #[test]
    fn 일한_뒤의_idle은_완료다() {
        let mut a = OpenCodeAdapter::default();
        assert_eq!(a.observe(&busy("s1")), Some(AgentState::Running));
        assert_eq!(a.observe(&idle_ev("s1")), Some(AgentState::Finished));
    }

    #[test]
    fn 일한_적_없는_idle은_아무_말도_하지_않는다() {
        // 붙자마자 오는 idle 로 🔴을 켜면 첫 화면이 빨간불 천지가 된다.
        let mut a = OpenCodeAdapter::default();
        assert_eq!(a.observe(&idle_ev("s1")), None);
    }

    #[test]
    fn 연달아_오는_idle이_완료를_지우지_않는다() {
        // 실측: opencode 는 한 턴 끝에 status{idle} 과 session.idle 을 둘 다 보낸다.
        // 두 번째가 ⚪를 내면 🔴이 1ms 만에 꺼진다 — 제품이 실패하는 지점이다.
        let mut a = OpenCodeAdapter::default();
        a.observe(&busy("s1"));
        let status_idle =
            r#"{"type":"session.status","properties":{"sessionID":"s1","status":{"type":"idle"}}}"#;
        assert_eq!(a.observe(status_idle), Some(AgentState::Finished));
        assert_eq!(
            a.observe(&idle_ev("s1")),
            None,
            "중복 idle 이 🔴을 지우면 안 된다"
        );
    }

    #[test]
    fn 승인_대기중의_busy는_노란불을_덮지_않는다() {
        let mut a = OpenCodeAdapter::default();
        a.observe(&busy("s1"));
        assert_eq!(a.observe(&ask("p1", "s1")), Some(AgentState::Waiting));
        assert_eq!(
            a.observe(&busy("s1")),
            None,
            "🟡을 🟢로 덮으면 프롬프트를 놓친다"
        );
    }

    #[test]
    fn 승인에_답하면_작업이_재개된다() {
        let mut a = OpenCodeAdapter::default();
        a.observe(&busy("s1"));
        a.observe(&ask("p1", "s1"));
        assert_eq!(a.observe(&replied("p1", "s1")), Some(AgentState::Running));
    }

    #[test]
    fn 승인이_여러_건이면_하나_답해도_노란불이다() {
        let mut a = OpenCodeAdapter::default();
        a.observe(&ask("p1", "s1"));
        a.observe(&ask("p2", "s1"));
        assert_eq!(a.observe(&replied("p1", "s1")), None);
        assert_eq!(a.observe(&replied("p2", "s1")), Some(AgentState::Running));
    }

    #[test]
    fn 세션이_끝나면_미응답_승인은_사라진다() {
        // 승인 프롬프트를 남긴 채 세션이 끝나면 🟡이 영원히 안 꺼진다.
        let mut a = OpenCodeAdapter::default();
        a.observe(&busy("s1"));
        a.observe(&ask("p1", "s1"));
        assert_eq!(a.observe(&idle_ev("s1")), Some(AgentState::Finished));
        assert_eq!(a.observe(&busy("s1")), Some(AgentState::Running));
    }

    #[test]
    fn 에러도_사용자가_봐야_한다() {
        let mut a = OpenCodeAdapter::default();
        a.observe(&busy("s1"));
        let ev = r#"data: {"type":"session.error","properties":{"sessionID":"s1","error":{}}}"#;
        assert_eq!(a.observe(ev), Some(AgentState::Finished));
    }

    #[test]
    fn global_event_의_감싼_봉투를_벗긴다() {
        // /global/event 는 {directory, project, payload:{...}} 로 감싼다. 실측 원문 그대로.
        let mut a = OpenCodeAdapter::default();
        let wrapped_busy = r#"data: {"directory":"/tmp/polycanv","project":"global","payload":{"id":"evt_1","type":"session.status","properties":{"sessionID":"s1","status":{"type":"busy"}}}}"#;
        let wrapped_idle = r#"data: {"directory":"/tmp/polycanv","project":"global","payload":{"id":"evt_2","type":"session.idle","properties":{"sessionID":"s1"}}}"#;
        assert_eq!(a.observe(wrapped_busy), Some(AgentState::Running));
        assert_eq!(a.observe(wrapped_idle), Some(AgentState::Finished));
    }

    #[test]
    fn 재시도중은_여전히_실행중이다() {
        let mut a = OpenCodeAdapter::default();
        let ev = r#"data: {"type":"session.status","properties":{"sessionID":"s1","status":{"type":"retry","attempt":2,"message":"429","next":1000}}}"#;
        assert_eq!(a.observe(ev), Some(AgentState::Running));
    }

    #[test]
    fn 관심없는_이벤트는_상태를_건드리지_않는다() {
        let mut a = OpenCodeAdapter::default();
        for line in [
            r#"data: {"id":"evt_0","type":"server.connected","properties":{}}"#,
            r#"data: {"type":"message.part.updated","properties":{"sessionID":"s1"}}"#,
            r#"data: {"type":"file.edited","properties":{"file":"a.rs"}}"#,
            ": heartbeat",
            "",
            "   ",
            "event: message",
            "data: not-json",
        ] {
            assert_eq!(a.observe(line), None, "{line}");
        }
    }

    #[test]
    fn sdk_타입의_옛_이름도_받는다() {
        // 서버 버전이 다르면 permission.updated / permissionID 로 올 수 있다.
        let mut a = OpenCodeAdapter::default();
        let asked = r#"{"type":"permission.updated","properties":{"id":"p9","sessionID":"s1"}}"#;
        let repl =
            r#"{"type":"permission.replied","properties":{"sessionID":"s1","permissionID":"p9"}}"#;
        assert_eq!(a.observe(asked), Some(AgentState::Waiting));
        assert_eq!(a.observe(repl), Some(AgentState::Running));
    }
}
