//! 실측 SSE 로그 재생 — 단위 테스트가 상상한 페이로드가 아니라 **실제로 받은 바이트**로 검증한다.
//!
//! 픽스처는 opencode 1.14.48 서버에 붙어 실제 프롬프트를 돌리며 받은 `/global/event` 원문이다
//! (경로만 익명화). 이 테스트가 깨지면 opencode 의 이벤트 형식이 바뀐 것이다 — 코드를 고치기 전에
//! 픽스처를 다시 뜨고 무엇이 달라졌는지부터 확인하라.

use polycanv_protocol::{AgentState, PaneKey};
use polycanv_status::ingress::Ingress;

const PANE: PaneKey = PaneKey::Terminal(1);

#[test]
fn 실측_로그를_재생하면_실행중_다음_완료가_난다() {
    let log = include_str!("fixtures/opencode-global-event.log");
    let mut ing = Ingress::new();
    let states: Vec<AgentState> = log
        .lines()
        .enumerate()
        .filter_map(|(i, line)| ing.ingest("opencode", PANE, line, i as u64))
        .map(|ev| ev.state)
        .collect();

    assert_eq!(
        states,
        vec![AgentState::Running, AgentState::Finished],
        "한 턴이면 신호는 딱 두 번이다. 그보다 많으면 사이드바가 깜빡거리고, 적으면 완료를 놓친다"
    );
    assert_eq!(
        ing.state(PANE),
        AgentState::Finished,
        "🔴은 포커스 전까지 남아 있어야 한다"
    );
}

#[test]
fn 실측_로그의_잡음_이벤트는_상태를_흔들지_않는다() {
    // message.part.updated 같은 것들이 상태를 건드리면 초당 수십 번 렌더가 돈다.
    let log = include_str!("fixtures/opencode-global-event.log");
    let noisy = log
        .lines()
        .filter(|l| !l.contains("\"session.status\"") && !l.contains("\"session.idle\""))
        .count();
    assert!(
        noisy > 0,
        "픽스처에 잡음 이벤트가 있어야 이 테스트가 의미가 있다"
    );

    let mut ing = Ingress::new();
    for (i, line) in log.lines().enumerate() {
        if line.contains("\"session.status\"") || line.contains("\"session.idle\"") {
            continue;
        }
        assert!(
            ing.ingest("opencode", PANE, line, i as u64).is_none(),
            "{line}"
        );
    }
    assert_eq!(ing.state(PANE), AgentState::Idle);
}

#[test]
fn 실측_승인_흐름을_재생하면_노란불이_제자리에_뜬다() {
    // busy → permission.asked → (사용자가 승인) → busy → idle 을 실제로 받은 원문.
    let log = include_str!("fixtures/opencode-permission-flow.log");
    let mut ing = Ingress::new();
    let states: Vec<AgentState> = log
        .lines()
        .enumerate()
        .filter_map(|(i, line)| ing.ingest("opencode", PANE, line, i as u64))
        .map(|ev| ev.state)
        .collect();

    assert_eq!(
        states,
        vec![
            AgentState::Running,
            AgentState::Waiting,
            AgentState::Running,
            AgentState::Finished
        ],
        "🟡이 빠지면 사용자가 승인 프롬프트를 놓치고, 🟢이 🟡을 덮어도 마찬가지다"
    );
}
