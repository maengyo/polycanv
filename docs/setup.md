# polycanv 실행 · 배선 안내 (개발 단계)

아직 설치 스크립트가 없다. 이 문서는 **지금 상태에서 직접 돌려보는 법**이다.
여기 적힌 것은 모두 macOS 에서 실측한 절차다. Windows / Linux 는 미검증이다.

## 1. 요구사항

| | 버전 | 비고 |
|---|---|---|
| zellij | **0.44.3** 이상 | `replace_pane_with_existing_pane` 때문에 최소 0.43.0, 0.44.3 권장 |
| rust | 1.97.1 (stable) | wasm 타깃 `wasm32-wasip1` 필요 |

```sh
brew install zellij rustup
rustup default stable
rustup target add wasm32-wasip1
```

## 2. 빌드 · 설치

```sh
cd <저장소>
cargo build --release --target wasm32-wasip1 -p polycanv-sidebar
mkdir -p ~/.config/zellij/plugins
cp target/wasm32-wasip1/release/polycanv-sidebar.wasm ~/.config/zellij/plugins/
```

`layouts/polycanv.kdl` 이 `file:~/.config/zellij/plugins/polycanv-sidebar.wasm` 를 참조한다
(`~` 는 zellij 가 확장한다 — 실측). **재빌드했으면 이 복사를 다시 해야 반영된다.**

> ⚠️ 디버그 프로파일로 빌드하지 마라. `target/` 이 수백 MB 씩 불어난다. 항상 `--release`.

## 3. 실행

```sh
zellij --config config/keybinds.kdl -s polycanv -n layouts/polycanv.kdl
```

## 4. ★ 최초 실행 — 권한 승인이 필요하다 ★

**처음 띄우면 사이드바가 아무 키에도 반응하지 않는다.** 고장이 아니다.
zellij 플러그인은 최초 실행 시 권한 승인을 요구하고, 승인 전에는 로드만 되고 동작하지 않는다.

사이드바 패인에 뜬 권한 요청에 **`y`** 로 승인하라. 요청하는 권한은 둘뿐이다:
`ReadApplicationState`(패인 목록 읽기), `ChangeApplicationState`(패인 배치 변경).

승인은 지속되므로 다음 실행부터는 필요 없다.

## 5. 조작

| 키 | 동작 | 어디서 |
|---|---|---|
| `Ctrl+y` | 캔버스 ↔ 리스트 토글 | **어디서나** (전역 키바인딩 → 플러그인 파이프) |
| `Ctrl+o` | **런처 열기** (도구 골라 새 패인에서 실행) | **어디서나** |
| `↑` `↓` / `k` `j` | 항목 이동 | 사이드바 포커스 시 |
| `Enter` / `Space` | 선택 → 메인으로 | 사이드바 포커스 시 |
| `1`~`9` | 번호로 바로 선택 | 사이드바 포커스 시 |
| `Tab` / `v` | 뷰 토글 | 사이드바 포커스 시 |

> 선택하면 포커스가 그 터미널로 넘어간다(바로 입력하라고). 그 뒤에는 사이드바 로컬 키가 안 먹으므로
> **되돌아올 때는 `Ctrl+y`** 를 쓴다.

## 6. 신호등 켜기 (상태 감지)

`scripts/polycanv-hook.sh` 가 CLI 훅을 받아 사이드바로 상태를 보낸다.
**배선하기 전까지 신호등은 전부 ⚪ 로 남는다** — 없는 상태를 지어내지 않기 때문이다.

### claude code — `~/.claude/settings.json`

```json
{
  "hooks": {
    "UserPromptSubmit": [{ "hooks": [{ "type": "command", "command": "<저장소>/scripts/polycanv-hook.sh" }] }],
    "Notification":     [{ "hooks": [{ "type": "command", "command": "<저장소>/scripts/polycanv-hook.sh" }] }],
    "Stop":             [{ "hooks": [{ "type": "command", "command": "<저장소>/scripts/polycanv-hook.sh" }] }]
  }
}
```

### codex cli — `$CODEX_HOME/config.toml`

```toml
[[hooks.user_prompt_submit]]
type = "command"
command = "<저장소>/scripts/polycanv-hook.sh"

[[hooks.permission_request]]
type = "command"
command = "<저장소>/scripts/polycanv-hook.sh"

[[hooks.stop]]
type = "command"
command = "<저장소>/scripts/polycanv-hook.sh"
```

⚠️ **codex 는 훅 신뢰(trust) 게이트가 있다.** 신뢰되지 않은 훅은 **오류도 경고도 없이 조용히
실행되지 않는다.** codex TUI 의 훅 화면에서 한 번 신뢰시키면 그 뒤로는 지속된다.
polycanv 는 `--dangerously-bypass-hook-trust` 를 붙이지 않는다 — 그건 사용자의 보안 결정이다.

## 7. 눈으로 확인할 것 (아직 자동 검증이 안 되는 항목)

zellij 는 플러그인 패인의 내용을 외부에 노출하지 않는다(`dump-screen` 이 내장 플러그인 포함
전부 빈 출력이다). **아래는 사람이 봐야 한다.**

- [ ] 사이드바에 터미널이 한 줄씩, **이름 / CLI 종류 / 작업 디렉터리 / 신호등** 순으로 보이는가
- [ ] 선택된 항목이 하이라이트되는가
- [ ] `Ctrl+y` 로 접었을 때 **보던 터미널이 메인에 그대로** 있는가 (맥락 보존)
- [ ] 다시 펼쳤을 때 **마지막 선택 터미널이 포커스**를 유지하는가
- [ ] 작업 디렉터리가 길 때 읽을 수 있게 줄어드는가
- [ ] 상태가 바뀔 때 사이드바가 **떨리지(flicker) 않는가**
- [ ] 훅 배선 후, 메인에 **없는** 터미널이 응답을 마치면 사이드바에 🔴 이 뜨는가
- [ ] 그 터미널을 실제로 열어보면 🔴 이 해제되는가

## 8-1. 런처 (실측 동작)

**`Ctrl+o` 로 연다.** 런처는 `config/keybinds.kdl` 의 `LaunchOrFocusPlugin` 블록에서 도구 목록을
읽는다. `tool_` 접두사가 붙은 키만 도구로 본다 — 다른 설정과 섞여도 안전하다.
**항목을 추가·수정하려면 그 블록만 고치면 된다.**

```kdl
bind "Ctrl o" {
    LaunchOrFocusPlugin "file:~/.config/zellij/plugins/polycanv-launcher.wasm" {
        floating true
        tool_claude   "claude"
        tool_codex    "codex"
        tool_opencode "opencode"
        tool_qwen     "qwen"
        tool_pwsh     "pwsh -NoLogo"
        tool_내도구    "my-cli --flag"
    }
}
```
목록에서 `↑↓` 또는 숫자키로 고르고 `Enter`. 고른 도구가 새 패인에서 실행된다(실측).

프리셋에 없는 도구도 **1급 시민**이다(실측: `tool_probe "echo ..."` 로 등록한 도구가 그대로 실행됐다).
도구 종류는 **이름이 아니라 실행 파일로** 판단하므로 항목 이름을 자유롭게 바꿔도 상태 감지가 깨지지 않는다.

스크립트에서 띄우려면 **대상 플러그인을 반드시 명시**한다:
```sh
zellij pipe --name polycanv:launch --plugin file:~/.config/zellij/plugins/polycanv-launcher.wasm -- claude
```
⚠️ `--plugin` 을 빼면 듣고 있는 모든 플러그인으로 브로드캐스트되고, 그중 하나라도 응답하지 않으면
파이프가 매달린다. 그리고 같은 wasm 의 인스턴스가 여러 개면 **설정이 없는 인스턴스가 응답할 수 있다**
— 그 경우 도구가 조용히 안 뜬다(zellij 로그에 이유가 남는다).

## 6-1. 상태 경로가 둘이다 — 하나는 조립됐고 하나는 아니다

**① 훅 기반 CLI (claude / codex / qwen) — 조립 완료.**
`scripts/polycanv-hook.sh` 가 **사이드바로 직접** 상태를 보낸다. 중간에 아무것도 필요 없다.
6장 훅 배선만 하면 신호등이 켜진다. (실제 claude 턴으로 🔴 까지 실증됨)

**② opencode (SSE) — 아직 조립되지 않았다.**
opencode 는 훅이 아니라 HTTP SSE 로 이벤트를 낸다. 원문 SSE 를 해석하려면 Rust 어댑터가 필요하고,
그건 `plugins/status` 플러그인 안에 있다. 경로는 이렇다:

```
opencode --port N
  → scripts(plugins/status/bridge/opencode-status-bridge.sh) 가 SSE 를 스트리밍
  → polycanv-status 플러그인이 해석
  → 사이드바로 전달
```

**조립 완료.** `polycanv-status` 는 `layouts/polycanv.kdl` 에 **숨긴 플로팅 패인**으로 얹혀 있다
(`hide_floating_panes true`) — 화면을 차지하지 않고 백그라운드로 돈다. 로드는 실측 확인했다.

브리지는 **런처가 자동으로 띄운다.** 도구에 `sidecar_<이름>` 을 짝지어 두면, 도구를 띄운 뒤
그 패인 id 를 `{pane}` 자리에 넣어 보조 프로세스를 함께 실행한다:

```kdl
tool_opencode    "opencode --port 47311"
sidecar_opencode "<저장소>/plugins/status/bridge/opencode-status-bridge.sh --port 47311 --pane {pane}"
```

포트를 고정하는 이유: opencode TUI 는 포트를 랜덤 배정하고 파일로 노출하지 않는다.
사이드카는 플로팅으로 떠서 `hide_floating_panes` 에 가려지므로 화면을 차지하지 않는다.

⚠️ 사이드카 파싱·치환은 단위 테스트로 검증됐지만 **실제 구동은 미검증**이다
(헤드리스에서 권한 승인 대화를 안정적으로 넘기지 못했다). 화면에서 직접 확인해야 한다.

수동으로 붙이려면:
```sh
plugins/status/bridge/opencode-status-bridge.sh --port 47311 --pane <opencode 패인의 ZELLIJ_PANE_ID>
```

## 7-1. 상태 감지 — 어디까지 실증됐나

**실제 claude code 훅이 사이드바 와이어 포맷까지 도달하는 것을 확인했다.**
zellij 패인 안에서 claude 를 돌리고, 훅 원문을 그대로 브리지에 통과시킨 결과:

```
claude 훅 (UserPromptSubmit, pane=19)
  → {"pane":{"terminal":19},"state":"running","source":"hook","at_ms":...}
```

패인 식별(`ZELLIJ_PANE_ID`)·상태 매핑·와이어 포맷이 실제 데이터로 맞물린다.

**`Stop`(🔴 완료)까지 확인했다.** 실제 인증된 claude 턴 한 번에서:
```
UserPromptSubmit (pane=21) → state="running"
Stop             (pane=21) → state="finished"   ← 🔴
```

> 💡 검증에 `claude --settings <파일>` 을 썼다 — **사용자의 `~/.claude/settings.json` 을 고치지 않고**
> 훅만 얹는 방법이다. 런처도 이 방식을 써야 한다(6장의 수동 편집은 임시 안내다).

## 8. 알려진 한계 / 미검증

- **신호등이 ⚪ 고정**이다 — 6장 배선을 해야 켜진다. 배선 후 동작은 미검증.
- **런처는 v0 이다.** 파이프 실행과 목록·선택 로직은 동작하지만(8-1 참조), 목록 UI 자체는
  플러그인 패인이라 화면 확인이 안 됐다. 도구 목록을 **별도 `config/tools.kdl` 로 빼는 것은
  미구현** — 현재는 레이아웃의 플러그인 블록에 쓴다.
- **PowerShell 실측 완료** (macOS 용 pwsh 7.6.5). polycanv 패인에서 실행되고 입력에 반응한다
  (`$PSVersionTable.PSVersion.Major` → `7`). polycanv 입장에서 셸은 **그냥 실행할 명령**이므로,
  Windows 의 pwsh 도 같은 경로를 탄다.
- **WSL 미검증.** Windows 전용이라 이 환경에서 흉내낼 수 없다. 다만 polycanv 가 하는 일은
  `wsl.exe` 또는 배포판 셸을 도구로 띄우는 것뿐이고, 그 경로는 pwsh 로 검증된 것과 동일하다.
- **AI CLI 4종 + 셸 동시 구동 실측 완료** (claude·codex·opencode·qwen·pwsh 5개 동시).
- **Windows 네이티브 미검증.**
- **"슈루룩" 애니메이션 없음.** 현재는 즉시 전환이다.
- ~~`zellij pipe` 가 반환하지 않는 문제~~ → **해결.** 원인은 플러그인이 `ReadCliPipes` 권한을
  요청하지 않아 `unblock_cli_pipe_input` 이 조용히 거부된 것이었다. 지금은 0초에 반환한다.
