//! zellij wasm 글루. 판단은 전부 라이브러리(`polycanv_sidebar`)에 있고 여기는 호출만 한다.
//!
//! 이 파일은 `cfg(target_arch = "wasm32")` 뒤에 격리돼 있다. zellij-tile 의 shim 은 호스트에
//! 없는 `host_run_plugin_command` 심볼을 링크하므로, 여기가 노출되면 `cargo test` 가 돌지 않는다.

use std::collections::{BTreeMap, BTreeSet};

use polycanv_protocol::{PaneKey, StatusEvent, StatusRecord, ViewMode};
use polycanv_sidebar::{
    build_rows, clamp_selection, drive, fold_targets, pick_main, refresh_targets, restore_targets,
    row_index_at_line, screen, step_selection, visible_window, Drive, PaneFacts, PaneSnapshot, Row,
    HEADER_ROWS,
};
use zellij_tile::prelude::*;

/// cwd·실행 명령을 다시 물어보는 주기(초). `PaneInfo` 에는 둘 다 없어서 직접 조회해야 한다.
const REFRESH_SECS: f64 = 2.0;

/// `plugins/status/` 가 상태 변화를 실어 보내는 파이프 이름 (`ingress::BROADCAST_NAME`).
const STATE_PIPE: &str = "polycanv:state";
/// 뷰 토글 요청. `config/keybinds.kdl` 의 `MessagePlugin` 이 이 이름으로 보낸다.
const TOGGLE_PIPE: &str = "toggle_view";
const LIST_PIPE: &str = "view_list";
const CANVAS_PIPE: &str = "view_canvas";

struct Pending {
    target: ViewMode,
    steps: u8,
    /// 도착한 뒤 포커스를 둘 패인. 맥락 보존(절대원칙 3)은 **양방향**이라 두 방향 모두 채운다.
    focus_after: Option<PaneKey>,
}

#[derive(Default)]
pub struct Sidebar {
    /// 자기 자신의 플러그인 패인 id. 이걸로 "내가 있는 탭"을 찾는다.
    own: Option<u32>,
    tab: usize,
    /// 내 탭의 터미널 패인. 접힌 것도 포함한다.
    panes: Vec<PaneSnapshot>,
    facts: BTreeMap<PaneKey, PaneFacts>,
    /// 패인별 상태 **기록**. 단순 값이 아니라 [`StatusRecord`] 인 이유:
    /// 출처 등급(훅 > 패턴 …)과 도착 순서를 따져 병합해야 하고, 사용자가 확인하면 🔴 을 풀어야 한다.
    /// 값만 들고 있으면 약한 출처가 훅이 세운 🔴 을 지워버린다.
    states: BTreeMap<PaneKey, StatusRecord>,
    /// **관리하는 상태는 사실상 이것 하나다** — 지금 메인 슬롯에 있는 패인.
    main: Option<PaneKey>,
    selected: usize,
    mode: ViewMode,
    /// 마지막으로 본 활성 swap layout 이름. **이 값이 바뀌는 순간이 "전환이 실제로 반영됐다"는
    /// 유일한 신호다** — `TabUpdate` 자체는 포커스·리사이즈·출력에도 오므로 신호가 못 된다.
    active_layout: Option<String>,
    pending: Option<Pending>,
    /// 마지막으로 그린 목록. 이것과 같으면 다시 그리지 않는다 — 깜빡임 방지의 전부다.
    view: Vec<Row>,
    /// 마지막 렌더의 스크롤 위치. 클릭 좌표를 줄로 바꿀 때 쓴다.
    offset: usize,
    /// 메타데이터 갱신 회전 커서. 틱마다 패인 하나씩 돌아가며 갱신한다.
    refresh_cursor: usize,
    /// 그린 프레임을 stderr 로도 내보낸다 (`debug_render "true"`).
    ///
    /// **zellij 는 플러그인 패인의 내용을 외부에 노출하지 않는다** — `dump-screen` 이
    /// 내장 플러그인까지 전부 빈 출력이다(실측). 그래서 사람이 화면을 보지 않으면
    /// "무엇을 그렸는지" 를 확인할 방법이 없었다.
    /// stderr 는 zellij 로그로 나가므로, 이 플래그가 그 구멍을 메운다.
    debug_render: bool,
}

impl ZellijPlugin for Sidebar {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        self.debug_render = configuration.get("debug_render").map(String::as_str) == Some("true");
        // 구독을 권한 요청보다 먼저 한다 (docs/research/zellij-pane-control-api.md 「재현 절차」).
        subscribe(&[
            EventType::PaneUpdate,
            EventType::TabUpdate,
            EventType::Key,
            EventType::Mouse,
            EventType::Timer,
            EventType::PermissionRequestResult,
        ]);
        request_permission(&[
            // 패인 목록·cwd·실행 명령 조회.
            PermissionType::ReadApplicationState,
            // 패인 교체 / suppressed 되돌리기 / swap layout.
            PermissionType::ChangeApplicationState,
            // CLI 파이프를 풀어주려면 필요하다. 없으면 unblock 이 조용히 거부되고
            // `zellij pipe` 호출자가 매달린다 (실측).
            PermissionType::ReadCliPipes,
        ]);
        self.own = Some(get_plugin_ids().plugin_id);
        set_selectable(true);
        set_timeout(REFRESH_SECS);
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::PermissionRequestResult(_) => {
                self.own = Some(get_plugin_ids().plugin_id);
                true
            }
            Event::PaneUpdate(manifest) => self.on_panes(manifest),
            Event::TabUpdate(tabs) => self.on_tabs(tabs),
            Event::Key(key) => self.on_key(key),
            Event::Mouse(mouse) => self.on_mouse(mouse),
            Event::Timer(_) => {
                set_timeout(REFRESH_SECS);
                // 레이아웃 이름이 끝내 안 바뀌면(스왑 레이아웃이 없는 탭 등) 여기서 매듭짓는다.
                // 이게 없으면 pending 이 영원히 남는다.
                let stepped = self.step_pending();
                self.refresh_facts();
                let rebuilt = self.rebuild();
                stepped || rebuilt
            }
            _ => false,
        }
    }

    fn pipe(&mut self, message: PipeMessage) -> bool {
        // ★ CLI 파이프는 **명시적으로 풀어주지 않으면 `zellij pipe` 가 반환하지 않는다.**
        //   (실측: 페이로드를 실어 보내면 3분 넘게 매달렸다. 페이로드 없는 파이프는 즉시 끝난다.)
        //   상태 훅 브리지가 여기서 막히면 CLI 의 턴이 통째로 멈춘다 — 신호등 하나 때문에
        //   사용자의 작업을 세우는 셈이다. 무엇을 하든 **먼저** 푼다.
        if let PipeSource::Cli(pipe_id) = &message.source {
            unblock_cli_pipe_input(pipe_id);
        }
        match message.name.as_str() {
            TOGGLE_PIPE => {
                self.set_mode(self.mode.toggled());
                true
            }
            LIST_PIPE => {
                self.set_mode(ViewMode::List);
                true
            }
            CANVAS_PIPE => {
                self.set_mode(ViewMode::Canvas);
                true
            }
            STATE_PIPE => self.on_status(message.payload.as_deref()),
            _ => false,
        }
    }

    fn render(&mut self, rows: usize, cols: usize) {
        let (offset, _) = visible_window(
            self.view.len(),
            self.selected,
            rows.saturating_sub(HEADER_ROWS),
        );
        self.offset = offset;
        let frame = screen(&self.view, self.mode, self.selected, offset, rows, cols);
        if self.debug_render {
            // 로그에서 프레임 경계를 찾기 쉽게 표식을 붙인다.
            eprintln!(
                "[frame] mode={:?} rows={rows} cols={cols} lines={}",
                self.mode,
                frame.len()
            );
            for (i, line) in frame.iter().enumerate() {
                eprintln!("[frame:{i:02}] {line}");
            }
        }
        for line in frame {
            println!("{line}");
        }
    }
}

impl Sidebar {
    // ── 이벤트 ────────────────────────────────────────────────────────────────

    fn on_panes(&mut self, manifest: PaneManifest) -> bool {
        let own = self.own;
        // 내가 들어 있는 탭만 본다. 다른 탭의 터미널을 메인에 올리는 건 v1 범위가 아니다.
        if let Some((tab, _)) = manifest
            .panes
            .iter()
            .find(|(_, panes)| panes.iter().any(|p| p.is_plugin && Some(p.id) == own))
        {
            self.tab = *tab;
        }
        let Some(panes) = manifest.panes.get(&self.tab) else {
            return false;
        };

        self.panes = panes
            .iter()
            .filter(|p| !p.is_plugin)
            .map(|p| PaneSnapshot {
                key: PaneKey::Terminal(p.id),
                title: p.title.clone(),
                is_suppressed: p.is_suppressed,
                is_focused: p.is_focused,
            })
            .collect();

        let alive: BTreeSet<PaneKey> = self.panes.iter().map(|p| p.key).collect();
        self.facts.retain(|k, _| alive.contains(k));
        self.states.retain(|k, _| alive.contains(k));
        // 새로 생긴 패인은 즉시 물어본다. 타이머를 기다리면 새 터미널이 2초 동안 빈 줄로 보인다.
        for key in &alive {
            if !self.facts.contains_key(key) {
                self.facts.insert(*key, facts_for(*key));
            }
        }

        // 사용자가 실제로 본 패인의 🔴 을 해제한다 (사양: "빨간불은 확인하면 해제").
        // 🟡 은 남긴다 — 승인 프롬프트는 쳐다본다고 사라지지 않는다. 규칙은 protocol 이 갖고 있다.
        if let Some(focused) = self.panes.iter().find(|p| p.is_focused).map(|p| p.key) {
            if let Some(rec) = self.states.get_mut(&focused) {
                rec.acknowledge(now_ms());
            }
        }

        self.main = pick_main(&self.panes, self.main);
        self.rebuild()
    }

    fn on_tabs(&mut self, tabs: Vec<TabInfo>) -> bool {
        let Some(info) = tabs.iter().find(|t| t.position == self.tab) else {
            return false;
        };
        let active = info.active_swap_layout_name.clone();
        let layout_moved = active != self.active_layout;
        self.active_layout = active;

        // 사용자가 zellij 기본 키로 직접 레이아웃을 돌렸을 수도 있다. 우리 믿음이 아니라
        // 실제 활성 레이아웃을 따른다.
        let observed = if self.active_layout.as_deref() == Some(polycanv_sidebar::LIST_LAYOUT) {
            ViewMode::List
        } else {
            ViewMode::Canvas
        };
        let mut changed = observed != self.mode;
        self.mode = observed;

        // ★ 레이아웃 이름이 **바뀌었을 때만** 다음 수를 둔다.
        //   `TabUpdate` 는 전환과 무관하게도 오고, 그때는 아직 직전 레이아웃 이름을 싣고 있다.
        //   그걸 "아직 도착 못 했다"로 읽으면 next_swap_layout() 을 한 번 더 불러
        //   목표를 지나쳐버린다 (실측: 캔버스 복귀가 상한까지 튕기다 포기했다).
        if layout_moved {
            changed |= self.step_pending();
        }
        let rebuilt = self.rebuild();
        changed || rebuilt
    }

    /// 뷰 전환 **2단계 — swap layout 몰기**. 한 수만 둔다.
    fn step_pending(&mut self) -> bool {
        let Some(p) = self.pending.take() else {
            return false;
        };
        match drive(p.target, self.active_layout.as_deref(), p.steps) {
            Drive::Arrived => {
                if let Some(key) = p.focus_after {
                    focus_pane_with_id(key.into(), false, false);
                }
                true
            }
            Drive::Step => {
                next_swap_layout();
                self.pending = Some(Pending {
                    steps: p.steps + 1,
                    ..p
                });
                false
            }
            Drive::GaveUp => {
                eprintln!(
                    "polycanv-sidebar: swap layout 을 {:?} 로 몰지 못했다 (active={:?}, 시도 {}회). \
                     목표 배치가 지금 패인 수에서 후보가 아닐 가능성이 크다 — 기저 레이아웃은 \
                     ExactPanes 제약이라 선언한 패인 수와 정확히 같을 때만 잡힌다. \
                     캔버스도 swap_tiled_layout 으로 두었는지 layouts/polycanv.kdl 을 확인하라.",
                    p.target, self.active_layout, p.steps
                );
                true
            }
        }
    }

    fn on_key(&mut self, key: KeyWithModifier) -> bool {
        match key.bare_key {
            BareKey::Up | BareKey::Char('k') => {
                self.selected = step_selection(self.selected, self.view.len(), false);
                true
            }
            BareKey::Down | BareKey::Char('j') => {
                self.selected = step_selection(self.selected, self.view.len(), true);
                true
            }
            BareKey::Enter | BareKey::Char(' ') => self.activate(self.selected),
            BareKey::Char(c @ '1'..='9') => {
                let index = c as usize - '1' as usize;
                self.activate(index)
            }
            // 사이드바에 포커스가 있을 때의 뷰 토글. 전역 키는 keybinds.kdl 의 MessagePlugin 이다.
            BareKey::Tab | BareKey::Char('v') => {
                self.set_mode(self.mode.toggled());
                true
            }
            _ => false,
        }
    }

    /// ⚠️ zellij 는 **포커스가 없는 패인의 첫 클릭을 포커스 이동으로 소비한다.** 플러그인에
    /// `Mouse` 이벤트가 오는 것은 그 다음 클릭부터다(실측). 사이드바만의 문제가 아니라
    /// zellij 의 마우스 모델이다 — 여기서 우회할 수 있는 것이 아니다.
    fn on_mouse(&mut self, mouse: Mouse) -> bool {
        match mouse {
            Mouse::LeftClick(line, _col) if line >= 0 => {
                match row_index_at_line(line as usize, self.view.len(), self.offset) {
                    Some(index) => self.activate(index),
                    None => false,
                }
            }
            Mouse::ScrollUp(_) => {
                self.selected = step_selection(self.selected, self.view.len(), false);
                true
            }
            Mouse::ScrollDown(_) => {
                self.selected = step_selection(self.selected, self.view.len(), true);
                true
            }
            _ => false,
        }
    }

    /// status 플러그인이 보낸 상태 변화. 없는 상태를 만들지 않고 온 것만 반영한다.
    fn on_status(&mut self, payload: Option<&str>) -> bool {
        let Some(payload) = payload else {
            return false;
        };
        let mut touched = false;
        for line in payload.lines() {
            if let Ok(ev) = serde_json::from_str::<StatusEvent>(line) {
                // 덮어쓰지 않고 **병합**한다 — 규칙은 protocol 이 갖고 있다.
                match self.states.entry(ev.pane) {
                    std::collections::btree_map::Entry::Occupied(mut e) => {
                        if e.get_mut().apply(&ev) {
                            touched = true;
                        }
                    }
                    std::collections::btree_map::Entry::Vacant(e) => {
                        e.insert(StatusRecord::new(ev.state, ev.source, ev.at_ms));
                        touched = true;
                    }
                }
                continue;
            }
        }
        touched && self.rebuild()
    }

    // ── 동작 ──────────────────────────────────────────────────────────────────

    /// 고른 항목을 메인으로. 리스트 뷰에서는 교체, 캔버스 뷰에서는 이미 다 보이므로 포커스만.
    fn activate(&mut self, index: usize) -> bool {
        let Some(row) = self.view.get(index) else {
            return false;
        };
        let target = row.key;
        self.selected = index;

        if self.mode == ViewMode::List && Some(target) != self.main {
            if let Some(main) = self.main {
                // ★ 세 번째 인자는 언제나 true. false 는 밀려난 패인을 **닫는다**.
                //   지오메트리는 이 호출 안에서 승계된다.
                replace_pane_with_existing_pane(main.into(), target.into(), true);
            }
        }
        // 포커스는 **직접 옮겨야 한다.** replace 내부의 move_clients_between_panes 는
        // "밀려나는 패인에 포커스가 있던 클라이언트"만 옮긴다. 사이드바를 클릭해서 고른 순간
        // 포커스는 사이드바에 있으므로 아무도 옮겨지지 않는다.
        // (실측: 이 호출 없이 클릭 후 타이핑하면 입력이 메인 패인에 닿지 않았다)
        focus_pane_with_id(target.into(), false, false);
        self.main = Some(target);
        self.rebuild();
        true
    }

    /// 뷰 전환 **1단계 — 패인 집합 맞추기**. 2단계(swap layout)는 [`Self::on_tabs`] 가 몬다.
    fn set_mode(&mut self, target: ViewMode) {
        if let Some(main) = self.main {
            match target {
                ViewMode::List => {
                    // 메인만 남기고 접는다. swap layout 은 suppressed 를 만들지 못한다.
                    for key in fold_targets(&self.panes, main) {
                        replace_pane_with_existing_pane(key.into(), main.into(), true);
                    }
                }
                ViewMode::Canvas => {
                    // 접힌 것을 타일로 되돌린다. swap layout 은 이것도 못 한다.
                    // should_focus_pane=false 라야 UnsuppressOrExpandPane 경로로 가서
                    // 포커스를 빼앗지 않고 조용히 타일에 다시 놓인다.
                    for key in restore_targets(&self.panes) {
                        show_pane_with_id(key.into(), false, false);
                    }
                }
            }
        }
        self.pending = Some(Pending {
            target,
            steps: 1,
            // 양방향 모두 메인으로 포커스를 되돌린다. 리스트로 갈 때도 필요하다 —
            // 접는 과정에서 메인 패인이 tiled 에서 뽑혔다 꽂히면서 포커스가 사이드바로 흘러간다
            // (실측: 캔버스→리스트 직후 클라이언트가 plugin_5 를 잡고 있었다).
            focus_after: self.main,
        });
        next_swap_layout();
    }

    // ── 내부 ──────────────────────────────────────────────────────────────────

    /// 패인 메타데이터(cwd·실행 명령)를 **조금씩** 갱신한다.
    ///
    /// 대상 선택은 [`refresh_targets`] 가 한다 — 왜 전부 훑으면 안 되는지는 그쪽 주석에 있다.
    /// 새로 생긴 패인은 여기가 아니라 `PaneUpdate` 처리에서 즉시 한 번 조회한다.
    fn refresh_facts(&mut self) {
        self.refresh_cursor = self.refresh_cursor.wrapping_add(1);
        for key in refresh_targets(&self.panes, self.refresh_cursor) {
            let next = facts_for(key);
            // 조회가 실패했다고 이미 알던 것을 지우지 않는다.
            if next != PaneFacts::default() {
                self.facts.insert(key, next);
            }
        }
    }

    /// 목록을 다시 만들고 **화면이 실제로 달라졌을 때만** true. 이게 깜빡임 방지의 핵심이다.
    fn rebuild(&mut self) -> bool {
        let rows = build_rows(&self.panes, &self.facts, &self.states, self.main);
        self.selected = clamp_selection(self.selected, rows.len());
        if rows == self.view {
            return false;
        }
        self.view = rows;
        true
    }
}

/// 유닉스 epoch 밀리초. 확인 시각 기록에 쓴다.
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn facts_for(key: PaneKey) -> PaneFacts {
    let id: PaneId = key.into();
    PaneFacts {
        cwd: get_pane_cwd(id).ok().map(|p| p.display().to_string()),
        command: get_pane_running_command(id).unwrap_or_default(),
    }
}
