//! zellij 글루. 여기에만 zellij 의존이 있다.

use std::collections::BTreeMap;

use polycanv_launcher::{parse_tools, Tool};
use zellij_tile::prelude::*;

/// 파이프로 도구를 띄운다. 페이로드는 도구 이름.
/// 키보드 없이도 실행할 수 있어야 스크립트·다른 플러그인이 런처를 쓸 수 있다.
const LAUNCH_PIPE: &str = "polycanv:launch";

#[derive(Default)]
struct Launcher {
    tools: Vec<Tool>,
    selected: usize,
}

register_plugin!(Launcher);

impl ZellijPlugin for Launcher {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        self.tools = parse_tools(&configuration);
        subscribe(&[EventType::Key, EventType::PermissionRequestResult]);
        // 패인을 여는 데 ChangeApplicationState,
        // CLI 파이프를 풀어주는 데 ReadCliPipes 가 필요하다.
        // ★ ReadCliPipes 를 빠뜨리면 `unblock_cli_pipe_input` 이 **조용히 거부되고**
        //   (로그에만 남는다) `zellij pipe` 를 호출한 쪽이 영원히 매달린다. 실측으로 확인한 함정이다.
        request_permission(&[
            // 새 패인을 열고 배치를 바꾼다.
            PermissionType::ChangeApplicationState,
            // ★ open_command_pane 에 필수. 빠뜨리면 호스트가 거부하고 **플러그인이 패닉한다**
            //   (실측: `permission 'RunCommands' denied` → wasm unreachable → 플러그인 죽음).
            //   권한 부족이 우아한 실패가 아니라 크래시라는 점을 기억해라.
            PermissionType::RunCommands,
            // CLI 파이프를 풀어준다. 없으면 `zellij pipe` 호출자가 매달린다.
            PermissionType::ReadCliPipes,
        ]);
    }

    fn pipe(&mut self, message: PipeMessage) -> bool {
        // CLI 파이프는 먼저 풀어준다 — 안 풀면 `zellij pipe` 가 반환하지 않아
        // 호출한 쪽(훅·스크립트)이 매달린다. 사이드바와 같은 이유다.
        if let PipeSource::Cli(pipe_id) = &message.source {
            unblock_cli_pipe_input(pipe_id);
        }
        if message.name != LAUNCH_PIPE {
            return false;
        }
        let Some(name) = message.payload.as_deref().map(str::trim) else {
            return false;
        };
        match self.tools.iter().position(|t| t.name == name) {
            Some(i) => self.launch(i),
            None => {
                // 없는 도구를 조용히 무시하면 사용자는 왜 안 뜨는지 알 수 없다.
                eprintln!(
                    "polycanv-launcher: '{name}' 라는 도구가 설정에 없다. \
                     레이아웃의 플러그인 블록에 tool_{name} \"<실행파일>\" 을 추가해라. \
                     현재 등록된 도구: {:?}",
                    self.tools.iter().map(|t| &t.name).collect::<Vec<_>>()
                );
                false
            }
        }
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::Key(key) => self.on_key(key),
            Event::PermissionRequestResult(_) => true,
            _ => false,
        }
    }

    fn render(&mut self, rows: usize, _cols: usize) {
        println!("도구를 고르세요 (↑↓ / 1-9 / Enter)");
        if self.tools.is_empty() {
            println!();
            println!("  설정된 도구가 없습니다.");
            println!("  레이아웃의 플러그인 블록에 tool_<이름> \"<실행파일>\" 을 추가하세요.");
            return;
        }
        for (i, tool) in self.tools.iter().take(rows.saturating_sub(1)).enumerate() {
            let marker = if i == self.selected { '>' } else { ' ' };
            println!("{marker} {}. {:<12} {}", i + 1, tool.name, tool.command);
        }
    }
}

impl Launcher {
    fn on_key(&mut self, key: KeyWithModifier) -> bool {
        match key.bare_key {
            BareKey::Up | BareKey::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
                true
            }
            BareKey::Down | BareKey::Char('j') => {
                if self.selected + 1 < self.tools.len() {
                    self.selected += 1;
                }
                true
            }
            BareKey::Enter => self.launch(self.selected),
            BareKey::Char(c @ '1'..='9') => {
                let index = c as usize - '1' as usize;
                self.launch(index)
            }
            _ => false,
        }
    }

    /// 새 패인에서 도구를 띄운다. 사이드카가 있으면 함께 띄운다.
    fn launch(&mut self, index: usize) -> bool {
        let Some(tool) = self.tools.get(index) else {
            return false;
        };
        self.selected = index;

        let mut command = CommandToRun::new(&tool.command);
        command.args = tool.args.clone();
        let pane_id = open_command_pane(command, BTreeMap::new());

        // 사이드카(상태 브리지 등)는 도구 패인의 id 를 알아야 한다. 그래서 도구를 먼저 띄우고,
        // 돌아온 id 를 `{pane}` 자리에 넣는다.
        if let Some(sidecar) = &tool.sidecar {
            match pane_id {
                Some(PaneId::Terminal(id)) => {
                    let mut sc = CommandToRun::new(&sidecar.command);
                    sc.args = sidecar.args_for(id);
                    // 플로팅으로 띄운다 — 레이아웃이 `hide_floating_panes` 로 숨기므로
                    // 사용자 화면을 차지하지 않는다. 필요하면 토글해서 볼 수 있다(디버깅용).
                    open_command_pane_floating(sc, None, BTreeMap::new());
                }
                other => {
                    // 도구 패인 id 를 못 받으면 브리지를 띄워도 어디를 감시할지 모른다.
                    // 조용히 넘어가면 신호등이 안 켜지는 이유를 알 수 없으므로 남긴다.
                    eprintln!(
                        "polycanv-launcher: '{}' 의 패인 id 를 받지 못해 사이드카를 띄우지 않았다 \
                         (받은 값: {other:?}). 상태 신호등이 켜지지 않는다.",
                        tool.name
                    );
                }
            }
        }
        true
    }
}
