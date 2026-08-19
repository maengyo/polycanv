//! 사이드바가 그리는 목록의 순수 모델과, 뷰 전환 때 어떤 패인을 건드릴지 고르는 규칙.

use std::collections::BTreeMap;

use polycanv_protocol::{
    color_for, AgentState, GroupColor, GroupKey, PaneKey, StatusRecord, ToolKind,
};

/// `PaneManifest` 에서 사이드바가 실제로 쓰는 것만 뽑은 것.
///
/// 순서는 `PaneManifest` 가 주는 순서가 아니라 **패인 id 오름차순**으로 고정한다.
/// 포커스나 suppressed 여부에 따라 줄 순서가 바뀌면 사용자가 방금 보던 항목이 움직여서
/// 숫자키를 잘못 누른다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneSnapshot {
    pub key: PaneKey,
    /// zellij 가 붙인 패인 제목. 사용자가 이름을 바꿨으면 그 이름이 여기로 온다.
    pub title: String,
    /// 접혀 있는가. 접혀 있어도 프로세스는 살아 있다.
    pub is_suppressed: bool,
    pub is_focused: bool,
}

/// OS 에 물어봐야 알 수 있는 것 (`get_pane_cwd` / `get_pane_running_command`).
///
/// `PaneInfo` 에는 cwd 도, 지금 돌고 있는 명령도 없다 — 제목은 사용자가 바꿔버릴 수 있어서
/// 도구 종류의 근거로 쓸 수 없다. 그래서 별도로 조회해 캐시한다.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PaneFacts {
    pub cwd: Option<String>,
    /// argv. 조회에 실패하면 빈 벡터다.
    pub command: Vec<String>,
}

/// 사이드바 한 줄.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub key: PaneKey,
    pub name: String,
    /// **모르면 `None`.** 모를 때 그럴듯한 도구 이름을 채워 넣지 않는다.
    pub tool: Option<ToolKind>,
    pub cwd: Option<String>,
    pub state: AgentState,
    /// 지금 메인 슬롯을 점유 중인가.
    pub is_main: bool,
    pub is_suppressed: bool,
    /// 이 세션이 속한 작업 흐름. cwd 를 모르면 `None`.
    pub group: Option<GroupKey>,
    /// 그룹의 색. 그룹이 없으면 `None` — 색을 지어내지 않는다.
    pub color: Option<GroupColor>,
    /// 이 줄이 그룹의 **첫 줄**인가. 사이드바는 여기에만 그룹 이름을 쓴다.
    pub starts_group: bool,
}

/// 인터프리터를 거쳐 실행되는 CLI 가 있다 (`node /usr/local/bin/claude`).
/// argv[0] 만 보면 전부 node 가 되므로 한 칸 더 본다.
const INTERPRETERS: &[&str] = &[
    "node", "bun", "deno", "python", "python3", "uv", "npx", "pnpm",
];

const SHELLS: &[&str] = &[
    "bash",
    "zsh",
    "sh",
    "fish",
    "pwsh",
    "powershell",
    "nu",
    "dash",
    "ksh",
    "tcsh",
];

fn basename(s: &str) -> &str {
    let s = s.rsplit(['/', '\\']).next().unwrap_or(s);
    // 로그인 셸은 `-zsh` 처럼 하이픈이 붙어서 온다.
    s.strip_prefix('-').unwrap_or(s).trim_end_matches(".exe")
}

/// 돌고 있는 명령에서 도구 종류를 읽는다. 모르면 `None`.
///
/// 여기 없는 도구도 1급 시민이어야 하므로 마지막은 [`ToolKind::Other`] 다
/// (`crates/protocol/src/pane.rs` 의 주석 참조).
pub fn tool_from_command(command: &[String]) -> Option<ToolKind> {
    let mut args = command.iter().map(|s| basename(s));
    let mut name = args.next()?;
    if INTERPRETERS.contains(&name) {
        if let Some(next) = args.find(|a| !a.starts_with('-')) {
            name = next;
        }
    }
    Some(match name {
        "" => return None,
        "claude" => ToolKind::ClaudeCode,
        "codex" => ToolKind::Codex,
        "opencode" => ToolKind::OpenCode,
        "qwen" => ToolKind::QwenCode,
        s if SHELLS.contains(&s) => ToolKind::Shell(s.to_string()),
        s => ToolKind::Other(s.to_string()),
    })
}

/// 목록 한 줄씩을 만든다. 플러그인 패인(사이드바 자기 자신 포함)은 애초에 들어오지 않는다.
pub fn build_rows(
    panes: &[PaneSnapshot],
    facts: &BTreeMap<PaneKey, PaneFacts>,
    states: &BTreeMap<PaneKey, StatusRecord>,
    main: Option<PaneKey>,
) -> Vec<Row> {
    let mut rows: Vec<Row> = panes
        .iter()
        .filter(|p| p.key.is_terminal())
        .map(|p| {
            let f = facts.get(&p.key);
            let name = if p.title.trim().is_empty() {
                match p.key {
                    PaneKey::Terminal(id) => format!("terminal {id}"),
                    PaneKey::Plugin(id) => format!("plugin {id}"),
                }
            } else {
                p.title.trim().to_string()
            };
            Row {
                key: p.key,
                name,
                tool: f.and_then(|f| tool_from_command(&f.command)),
                cwd: f.and_then(|f| f.cwd.clone()),
                // status 플러그인이 아직 말해준 적 없으면 ⚪ 다. 추측하지 않는다.
                state: states.get(&p.key).map(|r| r.state).unwrap_or_default(),
                is_main: Some(p.key) == main,
                is_suppressed: p.is_suppressed,
                group: None,
                color: None,
                starts_group: false,
            }
        })
        .collect();
    // ★ 그룹으로 먼저 묶고 그 안에서 패인 순서. 그래야 같은 작업의 세션이 **붙어서** 읽힌다.
    //   흩어져 있으면 색을 칠해도 눈이 따라가지 못한다.
    //
    //   그룹이 없는(cwd 를 모르는) 세션은 맨 아래로 보낸다 — 정체가 불분명한 것이
    //   목록 한가운데서 묶음을 끊으면 안 된다.
    for row in &mut rows {
        row.group = GroupKey::from_cwd(row.cwd.as_deref());
        row.color = row.group.as_ref().map(color_for);
    }
    rows.sort_by(|a, b| match (&a.group, &b.group) {
        (Some(x), Some(y)) => x.cmp(y).then(a.key.cmp(&b.key)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.key.cmp(&b.key),
    });

    // 그룹의 첫 줄에만 표식을 남긴다. 같은 이름을 매 줄 반복하면 폭만 먹는다.
    let mut previous: Option<GroupKey> = None;
    for row in &mut rows {
        row.starts_group = match (&row.group, &previous) {
            (Some(g), Some(prev)) => g != prev,
            (Some(_), None) => true,
            (None, _) => false,
        };
        previous = row.group.clone();
    }
    rows
}

/// 메인 슬롯을 점유할 패인을 고른다.
///
/// 캔버스→리스트 전환에서 **그때 포커스돼 있던 패인이 메인으로** 가야 하므로(절대원칙 3)
/// 포커스가 최우선이다. 사이드바를 클릭한 직후처럼 포커스된 터미널이 없으면 직전 값을 지킨다.
pub fn pick_main(panes: &[PaneSnapshot], current: Option<PaneKey>) -> Option<PaneKey> {
    if let Some(p) = panes
        .iter()
        .find(|p| p.is_focused && !p.is_suppressed && p.key.is_terminal())
    {
        return Some(p.key);
    }
    if let Some(c) = current {
        if panes.iter().any(|p| p.key == c) {
            return Some(c);
        }
    }
    panes
        .iter()
        .filter(|p| p.key.is_terminal())
        .find(|p| !p.is_suppressed)
        .or_else(|| panes.iter().find(|p| p.key.is_terminal()))
        .map(|p| p.key)
}

/// 리스트 뷰로 갈 때 접을 패인. 메인은 남기고 나머지 **타일에 있는** 터미널 전부.
///
/// swap layout 은 tiled 만 재배치하므로, 이걸 먼저 접지 않으면 메인 영역이 N등분될 뿐이다.
pub fn fold_targets(panes: &[PaneSnapshot], main: PaneKey) -> Vec<PaneKey> {
    panes
        .iter()
        .filter(|p| p.key.is_terminal() && !p.is_suppressed && p.key != main)
        .map(|p| p.key)
        .collect()
}

/// 캔버스 뷰로 갈 때 타일로 되돌릴 패인. swap layout 은 suppressed 를 끌어올리지 못한다.
pub fn restore_targets(panes: &[PaneSnapshot]) -> Vec<PaneKey> {
    panes
        .iter()
        .filter(|p| p.key.is_terminal() && p.is_suppressed)
        .map(|p| p.key)
        .collect()
}

/// 이번 틱에 메타데이터를 다시 물어볼 패인들.
///
/// ★ **전부 훑으면 안 된다.** `get_pane_cwd` / `get_pane_running_command` 는 호스트 왕복이고
///   zellij 쪽 타임아웃이 **100ms** 다. 패인 N개를 매 틱 훑으면 틱당 2N 번의 블로킹 호출이 되어
///   전부 타임아웃으로 무너지고, **아이템의 CLI 종류·작업 디렉터리가 빈 채로 남는다.**
///   실측: 패인 30여 개 세션에서 `GetPaneRunningCommand timed out` 44,720회.
///
/// 그래서 **틱당 최대 2개**로 묶는다:
/// - 포커스된 패인 — 사용자가 `cd` 하는 곳이라 실제로 변한다
/// - 회전 커서가 가리키는 패인 하나 — 나머지도 결국 갱신된다
pub fn refresh_targets(panes: &[PaneSnapshot], cursor: usize) -> Vec<PaneKey> {
    if panes.is_empty() {
        return Vec::new();
    }
    let rotating = panes[cursor % panes.len()].key;
    let focused = panes.iter().find(|p| p.is_focused).map(|p| p.key);
    match focused {
        Some(f) if f != rotating => vec![f, rotating],
        Some(f) => vec![f],
        None => vec![rotating],
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn 같은_작업_디렉터리의_세션은_붙어서_나온다() {
        // 흩어져 있으면 색을 칠해도 눈이 따라가지 못한다 — 묶임이 먼저다.
        let panes = vec![
            pane(1, false, false),
            pane(2, false, false),
            pane(3, false, false),
        ];
        let mut facts = BTreeMap::new();
        facts.insert(PaneKey::Terminal(1), facts_with_cwd("/w/api"));
        facts.insert(PaneKey::Terminal(2), facts_with_cwd("/w/web"));
        facts.insert(PaneKey::Terminal(3), facts_with_cwd("/w/api"));

        let rows = build_rows(&panes, &facts, &BTreeMap::new(), None);
        let groups: Vec<_> = rows
            .iter()
            .map(|r| r.group.as_ref().unwrap().0.as_str())
            .collect();
        assert_eq!(
            groups,
            vec!["/w/api", "/w/api", "/w/web"],
            "같은 디렉터리가 붙어야 한다"
        );
    }

    #[test]
    fn 그룹의_첫_줄에만_머리글_표식이_붙는다() {
        let panes = vec![
            pane(1, false, false),
            pane(2, false, false),
            pane(3, false, false),
        ];
        let mut facts = BTreeMap::new();
        facts.insert(PaneKey::Terminal(1), facts_with_cwd("/w/api"));
        facts.insert(PaneKey::Terminal(2), facts_with_cwd("/w/api"));
        facts.insert(PaneKey::Terminal(3), facts_with_cwd("/w/web"));

        let rows = build_rows(&panes, &facts, &BTreeMap::new(), None);
        let starts: Vec<bool> = rows.iter().map(|r| r.starts_group).collect();
        assert_eq!(
            starts,
            vec![true, false, true],
            "같은 이름을 매 줄 반복하면 폭만 먹는다"
        );
    }

    #[test]
    fn 같은_그룹은_같은_색을_받는다() {
        let panes = vec![pane(1, false, false), pane(2, false, false)];
        let mut facts = BTreeMap::new();
        facts.insert(PaneKey::Terminal(1), facts_with_cwd("/w/api"));
        facts.insert(PaneKey::Terminal(2), facts_with_cwd("/w/api"));

        let rows = build_rows(&panes, &facts, &BTreeMap::new(), None);
        assert_eq!(rows[0].color, rows[1].color);
        assert!(rows[0].color.is_some());
    }

    #[test]
    fn cwd_를_모르는_세션은_맨_아래로_간다() {
        // 정체가 불분명한 것이 목록 한가운데서 묶음을 끊으면 안 된다.
        let panes = vec![pane(1, false, false), pane(2, false, false)];
        let mut facts = BTreeMap::new();
        facts.insert(PaneKey::Terminal(1), PaneFacts::default()); // cwd 없음
        facts.insert(PaneKey::Terminal(2), facts_with_cwd("/w/api"));

        let rows = build_rows(&panes, &facts, &BTreeMap::new(), None);
        assert_eq!(rows[0].key, PaneKey::Terminal(2), "그룹 있는 세션이 위로");
        assert!(rows[1].group.is_none());
        assert!(
            rows[1].color.is_none(),
            "그룹이 없으면 색도 지어내지 않는다"
        );
    }

    fn facts_with_cwd(cwd: &str) -> PaneFacts {
        PaneFacts {
            cwd: Some(cwd.to_string()),
            ..PaneFacts::default()
        }
    }

    #[test]
    fn 갱신_대상은_틱당_최대_두_개다() {
        // 패인이 몇 개든 호스트 왕복은 상수여야 한다 — 이게 타임아웃 붕괴를 막는다.
        let many: Vec<PaneSnapshot> = (0..50).map(|i| pane(i, false, false)).collect();
        for cursor in 0..60 {
            assert!(refresh_targets(&many, cursor).len() <= 2, "cursor={cursor}");
        }
    }

    #[test]
    fn 포커스된_패인은_항상_갱신한다() {
        // 사용자가 cd 하는 곳이라 가장 자주 변한다.
        let panes = vec![
            pane(1, false, false),
            pane(2, false, true),
            pane(3, false, false),
        ];
        for cursor in 0..6 {
            assert!(
                refresh_targets(&panes, cursor).contains(&PaneKey::Terminal(2)),
                "cursor={cursor}"
            );
        }
    }

    #[test]
    fn 회전_커서가_모든_패인을_돌아간다() {
        // 포커스 밖 패인도 결국 갱신돼야 한다 — 안 그러면 영영 낡은 값이 남는다.
        let panes = vec![
            pane(1, false, true),
            pane(2, false, false),
            pane(3, false, false),
        ];
        let mut seen = std::collections::BTreeSet::new();
        for cursor in 0..3 {
            seen.extend(refresh_targets(&panes, cursor));
        }
        assert!(seen.contains(&PaneKey::Terminal(2)));
        assert!(seen.contains(&PaneKey::Terminal(3)));
    }

    #[test]
    fn 패인이_없으면_아무것도_묻지_않는다() {
        assert!(refresh_targets(&[], 0).is_empty());
    }

    use super::*;

    fn pane(id: u32, suppressed: bool, focused: bool) -> PaneSnapshot {
        PaneSnapshot {
            key: PaneKey::Terminal(id),
            title: format!("t{id}"),
            is_suppressed: suppressed,
            is_focused: focused,
        }
    }

    #[test]
    fn 인터프리터를_거쳐도_진짜_도구를_찾는다() {
        let cmd: Vec<String> = ["node", "--enable-source-maps", "/opt/homebrew/bin/claude"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(tool_from_command(&cmd), Some(ToolKind::ClaudeCode));
    }

    #[test]
    fn 로그인셸은_하이픈이_붙어도_셸이다() {
        let cmd = vec!["-zsh".to_string()];
        assert_eq!(tool_from_command(&cmd), Some(ToolKind::Shell("zsh".into())));
    }

    #[test]
    fn 모르는_도구는_other_로_1급_시민이다() {
        let cmd = vec!["/usr/bin/htop".to_string()];
        assert_eq!(
            tool_from_command(&cmd),
            Some(ToolKind::Other("htop".into()))
        );
    }

    #[test]
    fn 명령을_모르면_도구도_none_이다() {
        // 조회 실패를 셸이라고 지어내면 사용자는 없는 정보를 믿게 된다.
        assert_eq!(tool_from_command(&[]), None);
    }

    #[test]
    fn 줄_순서는_패인_id_순이고_접힘_여부에_흔들리지_않는다() {
        let panes = vec![
            pane(3, true, false),
            pane(1, false, true),
            pane(2, true, false),
        ];
        let rows = build_rows(&panes, &BTreeMap::new(), &BTreeMap::new(), None);
        let ids: Vec<_> = rows.iter().map(|r| r.key).collect();
        assert_eq!(
            ids,
            vec![
                PaneKey::Terminal(1),
                PaneKey::Terminal(2),
                PaneKey::Terminal(3)
            ]
        );
    }

    #[test]
    fn 상태를_아무도_안_알려주면_흰불이다() {
        let rows = build_rows(
            &[pane(1, false, true)],
            &BTreeMap::new(),
            &BTreeMap::new(),
            None,
        );
        assert_eq!(rows[0].state, AgentState::Idle);
        assert_eq!(rows[0].tool, None);
        assert_eq!(rows[0].cwd, None);
    }

    #[test]
    fn 플러그인_패인은_목록에_없다() {
        let panes = vec![
            PaneSnapshot {
                key: PaneKey::Plugin(1),
                title: "sidebar".into(),
                is_suppressed: false,
                is_focused: true,
            },
            pane(1, false, false),
        ];
        let rows = build_rows(&panes, &BTreeMap::new(), &BTreeMap::new(), None);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].key, PaneKey::Terminal(1));
    }

    #[test]
    fn 메인은_포커스된_터미널이다() {
        let panes = vec![pane(1, false, false), pane(2, false, true)];
        assert_eq!(
            pick_main(&panes, Some(PaneKey::Terminal(1))),
            Some(PaneKey::Terminal(2))
        );
    }

    #[test]
    fn 사이드바를_클릭해_포커스가_없으면_직전_메인을_지킨다() {
        let panes = vec![pane(1, false, false), pane(2, true, false)];
        assert_eq!(
            pick_main(&panes, Some(PaneKey::Terminal(2))),
            Some(PaneKey::Terminal(2))
        );
    }

    #[test]
    fn 메인이_사라지면_보이는_패인으로_넘어간다() {
        let panes = vec![pane(5, false, false)];
        assert_eq!(
            pick_main(&panes, Some(PaneKey::Terminal(9))),
            Some(PaneKey::Terminal(5))
        );
    }

    #[test]
    fn 패인이_하나면_접을_것도_되돌릴_것도_없다() {
        let panes = vec![pane(1, false, true)];
        assert!(fold_targets(&panes, PaneKey::Terminal(1)).is_empty());
        assert!(restore_targets(&panes).is_empty());
    }

    #[test]
    fn 리스트로_갈_때_메인만_남기고_전부_접는다() {
        let panes: Vec<_> = (1..=6).map(|i| pane(i, false, i == 2)).collect();
        let fold = fold_targets(&panes, PaneKey::Terminal(2));
        assert_eq!(fold.len(), 5);
        assert!(!fold.contains(&PaneKey::Terminal(2)));
    }

    #[test]
    fn 캔버스로_갈_때_접힌_것만_되돌린다() {
        let panes = vec![
            pane(1, false, true),
            pane(2, true, false),
            pane(3, true, false),
        ];
        assert_eq!(
            restore_targets(&panes),
            vec![PaneKey::Terminal(2), PaneKey::Terminal(3)]
        );
    }
}
