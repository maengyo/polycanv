//! zellij wasm 글루. 판별 로직은 전혀 없다 — 전부 [`crate::ingress`] 로 넘긴다.
//!
//! 이 파일은 `cfg(target_arch = "wasm32")` 뒤에 격리돼 있다. zellij-tile 의 shim 은
//! 호스트에 없는 `host_run_plugin_command` 심볼을 링크하므로, 여기가 노출되면
//! `cargo test` 자체가 돌지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::time::{SystemTime, UNIX_EPOCH};

use polycanv_protocol::{AgentState, PaneKey};
use zellij_tile::prelude::*;

use crate::ingress::{Ingress, ARG_PANE, ARG_TOOL, BROADCAST_NAME, PIPE_NAME};

#[derive(Default)]
struct StatusPlugin {
    ingress: Ingress,
    /// 지금 살아 있는 터미널 패인. 사라진 패인의 맥락을 버리는 데 쓴다.
    alive: BTreeSet<PaneKey>,
    /// 활성 탭 위치. `is_focused` 는 탭마다 하나씩 참이라, 이게 없으면
    /// 보고 있지도 않은 탭의 🔴이 저절로 꺼진다.
    active_tab: Option<usize>,
    /// 마지막으로 🔴 해제를 처리한 패인. 같은 PaneUpdate 가 반복돼도 중복 처리하지 않는다.
    last_focus: Option<PaneKey>,
}

register_plugin!(StatusPlugin);

impl ZellijPlugin for StatusPlugin {
    fn load(&mut self, _configuration: BTreeMap<String, String>) {
        request_permission(&[
            // PaneUpdate/TabUpdate 로 포커스와 패인 생명주기를 읽는다.
            PermissionType::ReadApplicationState,
            // 사이드바로 상태 변화를 넘긴다.
            PermissionType::MessageAndLaunchOtherPlugins,
            // ★ CLI 파이프(브리지)를 풀어주는 데 필요하다. 없으면 `unblock_cli_pipe_input` 이
            //   **조용히 거부되고**(로그에만 남는다) 브리지가 첫 이벤트 뒤로 막힌다.
            //   opencode 브리지는 SSE 를 STDIN 스트림으로 흘리므로 이게 없으면 무용지물이다.
            PermissionType::ReadCliPipes,
        ]);
        subscribe(&[EventType::PaneUpdate, EventType::TabUpdate]);
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::TabUpdate(tabs) => {
                self.active_tab = tabs.iter().find(|t| t.active).map(|t| t.position);
                false
            }
            Event::PaneUpdate(manifest) => self.on_panes(manifest),
            _ => false,
        }
    }

    fn pipe(&mut self, message: PipeMessage) -> bool {
        // ★ 무엇을 하든 **먼저** 푼다. 브리지는 SSE 를 계속 흘려보내는 스트림이라,
        //   여기서 풀지 않으면 다음 줄이 오지 않는다 (실측으로 확인된 함정).
        //   이름이 안 맞아 무시할 이벤트여도 마찬가지다 — 막힌 파이프는 그대로 막혀 있다.
        if let PipeSource::Cli(pipe_id) = &message.source {
            unblock_cli_pipe_input(pipe_id);
        }
        if message.name != PIPE_NAME {
            return false;
        }
        // payload 가 None 이면 파이프가 끝났다는 뜻이다 (브리지 종료). 상태는 그대로 둔다 —
        // 브리지가 죽었다고 해서 터미널의 마지막 상태가 거짓이 되는 것은 아니다.
        let Some(payload) = message.payload.as_deref() else {
            return false;
        };
        let Some(tool) = message.args.get(ARG_TOOL) else {
            return false;
        };
        let Some(pane) = message
            .args
            .get(ARG_PANE)
            .and_then(|s| s.parse().ok())
            .map(PaneKey::Terminal)
        else {
            return false;
        };

        let now = now_ms();
        let mut changed = false;
        // `zellij pipe` 는 STDIN 을 흘려보낼 때 여러 줄이 한 payload 로 뭉쳐 올 수 있다.
        for line in payload.lines() {
            if let Some(ev) = self.ingress.ingest(tool, pane, line, now) {
                broadcast(&ev);
                changed = true;
            }
        }
        changed
    }

    fn render(&mut self, rows: usize, _cols: usize) {
        // 이 플러그인은 배경에서 도는 감지기다. 화면은 디버그용이므로 최소한만 그린다.
        for (pane, state) in self.ingress.snapshot().take(rows.saturating_sub(1)) {
            let PaneKey::Terminal(id) = pane else {
                continue;
            };
            println!("{} pane {}  {:?}", state.glyph(), id, state);
        }
    }
}

impl StatusPlugin {
    fn on_panes(&mut self, manifest: PaneManifest) -> bool {
        let mut seen = BTreeSet::new();
        let mut focused = None;

        for (tab, panes) in &manifest.panes {
            for pane in panes {
                if pane.is_plugin {
                    continue;
                }
                let key = PaneKey::Terminal(pane.id);
                seen.insert(key);
                // suppressed 패인은 화면에 없다. 사용자가 봤을 리 없으므로 🔴을 해제하면 안 된다.
                if pane.is_focused && !pane.is_suppressed && Some(*tab) == self.active_tab {
                    focused = Some(key);
                }
            }
        }

        let mut changed = false;
        for gone in self.alive.difference(&seen) {
            self.ingress.forget(*gone);
            changed = true;
        }
        self.alive = seen;

        if focused != self.last_focus {
            self.last_focus = focused;
            if let Some(key) = focused {
                if self.ingress.focus(key, now_ms()) {
                    broadcast_state(key, self.ingress.state(key));
                    changed = true;
                }
            }
        }
        changed
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn broadcast(ev: &polycanv_protocol::StatusEvent) {
    if let Ok(json) = serde_json::to_string(ev) {
        pipe_message_to_plugin(MessageToPlugin::new(BROADCAST_NAME).with_payload(json));
    }
}

fn broadcast_state(pane: PaneKey, state: AgentState) {
    broadcast(&polycanv_protocol::StatusEvent {
        pane,
        state,
        source: polycanv_protocol::StatusSource::Hook,
        at_ms: now_ms(),
    });
}
