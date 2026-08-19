# Multi-CLI Terminal Canvas (코드네임: polycanv)

여러 AI 코딩 CLI(claude code / codex cli / opencode / qwen code)와 셸(PowerShell / WSL Ubuntu)을
한 화면에 동시에 띄우고, 클릭으로 전환하며, 어떤 세션이 응답을 마쳤는지 신호등으로 파악하는
**Zellij 기반 TUI 오픈소스 도구**.

## 절대 원칙

1. **PTY·멀티플랫폼·세션영속성을 직접 구현하지 않는다.** Zellij가 이미 제공한다.
   직접 구현하려는 충동이 들면 그건 설계가 틀렸다는 신호다.
2. **뷰 전환은 프로세스를 죽이지 않는다.** 캔버스↔리스트는 배치 교체(swap layout)일 뿐이다.
   재시작·종료가 필요한 설계는 요구사항 위반이다.
3. **맥락 보존은 양방향이다.** 캔버스→리스트 시 선택돼 있던 패인이 메인으로,
   리스트→캔버스 시 마지막 선택 패인이 포커스를 유지한다.
4. **리스트 뷰는 끄는 게 아니라 접는 것이다.** 메인에 없는 터미널도 계속 실행되고
   상태 변화 시 사이드바 신호등이 즉시 갱신된다.

## 아키텍처

Zellij(Rust) 엔진 위에 WASM 플러그인 3종 + KDL 레이아웃 2종.

```
layouts/            캔버스·리스트 KDL (swap layout 쌍)
config/             keybinds.kdl, tools.kdl (런처 도구 목록)
crates/protocol/    ★공유 계약★ 상태 이벤트 타입 + 패인 메타데이터
plugins/launcher/   도구 선택 → 새 패인 실행
plugins/status/     running/waiting/finished/idle 판별 → 신호등
plugins/sidebar/    리스트 사이드바 렌더링, 선택 → 메인 패인 교체
scripts/            빌드·설치
docs/research/      조사·스파이크 결과 (zellij-scout 전용)
```

**상태 판별 우선순위**: ① CLI 훅/이벤트 → ② 출력 패턴 정규식 → ③ 벨 문자(\a) → ④ idle 휴리스틱.
상위 수단이 있으면 하위로 내려가지 않는다.

**신호등**: 🟢running / 🟡waiting(권한 승인 등 사용자 입력 필요) / 🔴finished(깜빡임) / ⚪idle.
🔴은 해당 터미널을 실제로 포커스하면 해제된다.

## 파일 소유 경계 (병렬 작업 충돌 방지)

| 에이전트 | 단독 소유 |
|---|---|
| zellij-scout | `docs/research/**` — 코드는 수정하지 않음 |
| layout-view | `layouts/**`, `config/keybinds.kdl` |
| launcher-plugin | `plugins/launcher/**`, `config/tools.kdl` |
| status-detector | `plugins/status/**` |
| sidebar-ui | `plugins/sidebar/**` |
| release-eng | `scripts/**`, `.github/**`, `docs/**`(research 제외), `README.md` |

**`crates/protocol/**` 는 리드(메인 세션)만 수정한다.** 하위 에이전트는 읽기 전용이며,
계약 변경이 필요하면 직접 고치지 말고 "protocol 변경 요청"으로 보고한다.
소유 경계 밖 파일이 필요하면 수정하지 말고 보고한다.

## 검증

- 근거 없이 "됐다"고 하지 않는다. `cargo build --target wasm32-wasi` 등 실제 명령 출력으로 확인한다.
- Zellij 플러그인 API는 버전에 따라 다르다. **기억이 아니라 실제 crate 소스/문서를 확인한다.**
- 산출물은 `/codex:rescue`(Codex 교차 검증)로 2차 확인한다.

## 미검증 리스크 (착수 전 확인 필요)

- ~~[높음] 리스트 선택 → 메인 패인 교체를 플러그인이 제어 가능한가~~ → **정적 판독으로 해소**.
  `replace_pane_with_existing_pane(교체될_패인, 올라올_패인, suppress_replaced_pane: true)` 한 번이면 된다.
  `true` 를 쓰면 밀려난 패인이 죽지 않고 suppressed 로 이동해 계속 실행되며 `PaneManifest` 에도 남는다.
  **`false` 는 패인을 닫는다 — polycanv에서는 절대 쓰지 마라.** permission은 `ChangeApplicationState`
  (+조회용 `ReadApplicationState`) 둘이면 충분하다. 최소 zellij 버전 **0.43.0**, 권장 **0.44.3**.
  근거·주의사항 전문: `docs/research/zellij-pane-control-api.md`
  ⚠️ **아직 런타임 검증 전이다** (스파이크 진행 중). 확정 전까지 이 API에 전부를 걸지 마라.
- ~~[높음] CLI별 상태 판별 훅·이벤트가 존재하는가~~ → **완전 해소**. 4개 CLI 전부 ①계층이다.
  - **claude code**: 훅 완전 구분 — `UserPromptSubmit`(running) / `Notification`+`PermissionRequest`(waiting) / `Stop`(finished)
  - **opencode**: HTTP SSE `GET /event` 완전 구분 — `session.status{busy}` / `permission.updated` / `session.idle`
  - **qwen code**: 문서상 claude 호환 훅. **로컬 미설치라 실측 없음**
  - **codex cli**: ✅ **훅 완전 구분 (TUI 실측)** — `UserPromptSubmit`(running) / `PermissionRequest`(waiting) / `Stop`(finished)
    1차 조사의 "미발화"는 훅 부재가 아니라 **신뢰(trust) 게이트** 탓이었다.
    신뢰되지 않은 훅은 오류도 경고도 없이 조용히 실행되지 않는다.
  실측 페이로드·재현 명령 전문: `docs/research/cli-status-hooks.md`
- **[중]** Zellij Windows 네이티브 지원(2026-03 출시)의 실제 동작 — 예제·문서 부족.
- **[중]** "슈루룩" 애니메이션의 TUI 표현 한계 → 미흡하면 즉시 전환 + 시각적 강조로 대체.

## 개발 환경 (2026-08-19 확인)

| 도구 | 버전 | 경로 |
|---|---|---|
| zellij | 0.44.3 | `/opt/homebrew/bin/zellij` |
| rustc / cargo | 1.97.1 | rustup 쉼(shim) `/opt/homebrew/opt/rustup/bin` |
| wasm 타깃 | `wasm32-wasip1` | 설치됨 |

rustup은 Homebrew keg-only라 `~/.zshrc` 에 PATH를 추가해 뒀다. 비대화형 셸에서 `cargo` 를 못 찾으면
`export PATH="/opt/homebrew/opt/rustup/bin:$PATH"` 를 앞에 붙여라.

**빌드 타깃은 `wasm32-wasip1` 이다.** 오래된 문서의 `wasm32-wasi` 는 이 툴체인에 없다.

⚠️ **디스크 여유가 5GB 남짓이다.** `target/` 이 빠르게 커지므로 큰 빌드 전에 `df -h` 로 확인하고,
필요하면 `cargo clean` 하라. 디스크가 차면 빌드가 링크 단계에서 이상한 오류로 실패한다.

## 확정된 설계 결정 (실측 근거 있음 — 뒤집지 마라)

### swap_tiled_layout 은 **캔버스·리스트 2개**다 (런타임 검증 완료)

기저 레이아웃을 캔버스 뷰로 쓰면 **리스트 → 캔버스 전환이 실패한다.**

zellij 는 기저를 `ExactPanes(선언된 패인 수)` 제약으로 등록한다
(`zellij-server/src/tab/swap_layouts.rs:47`, 주석 원문 *"the base layout is not intended to be
progressive"*). 패인 수가 선언과 다르면 **기저가 순환에서 통째로 빠지고**, 순환에 리스트만
남아 `next_swap_layout()` 을 몇 번 불러도 리스트에 고착된다.

→ **캔버스를 제약 없는 `swap_tiled_layout name="canvas"` 로 따로 선언한다.**
   기저는 그대로 두되 캔버스 뷰의 근거로 삼지 않는다.

실측 (리드, 패인 수를 바꿔가며 `list-tabs --all` 의 SWAP_LAYOUT 추적):
```
패인 5개(기저 선언과 일치):  BASE → canvas → list → BASE      (주기 3)
패인 6개(기저 탈락):              canvas → list → canvas      (주기 2)
```
**패인 수와 무관하게 캔버스에 도달한다** — 이것이 이 구조를 쓰는 이유다.

### 뷰 전환은 이름으로 몰아야 한다

`next_swap_layout()` 에는 인덱스 지정이 없고, 위처럼 **순환 길이가 패인 수에 따라 바뀐다.**
따라서 한 번 호출로 구현하면 안 된다.

- `TabInfo.active_swap_layout_name` 을 읽어 **목표에 닿을 때까지 반복 호출**하고 **상한을 둔다.**
- **캔버스는 이름이 아니라 "리스트가 아님"으로 판정한다.** 패인 수에 따라 `"BASE"` 로도
  `"canvas"` 로도 잡히기 때문이다. 이름을 하나로 못 박으면 그 순간 토글이 멈춘다.

### 레이아웃으로 되는 것 / 플러그인 호출이 필요한 것

**레이아웃만으로 된다**: 캔버스↔리스트 배치 전환, 사이드바를 항상 제자리로 되돌리기,
전환 중 프로세스 보존(왕복 10회 PID 동일).

**플러그인 호출이 필요하다**:
- 리스트 뷰에서 "메인 1개만 남기고 나머지 접기" — swap layout 은 tiled 만 재배치하고
  suppressed 를 건드리지 않는다. 타일에 N개가 남으면 메인 영역이 N등분될 뿐이다.
- 캔버스로 돌아갈 때 suppressed 를 타일로 되돌리기 — swap layout 은 끌어올리지 못한다.
- "리스트로 가라"를 결정론적으로 지정 — `next_swap_layout()` 은 인덱스 지정이 없다.
  `TabInfo.active_swap_layout_name` 을 읽어 목표에 닿을 때까지 반복 호출하면 결정론이 된다.

**즉 뷰 전환은 "플러그인이 패인 집합을 먼저 맞추고 → swap layout 을 호출"하는 2단계다.**
레이아웃만으로 완성하려 들지 마라.

### 레이아웃 작성 금지 사항

- 터미널 슬롯에 `command` 금지 — **기저 레이아웃에도** 금지다(기저도 순환의 일부다).
  exact match 에 걸리면 뷰 전환이 "지금 메인에 뭐가 있는지" 상태를 덮어쓴다. 터미널은 런처가 채운다.
- 사이드바 슬롯에는 `plugin location=` 명시 — 이게 사이드바를 항상 제자리로 되돌린다.
- `max_panes` / `min_panes` / `exact_panes` 금지 — 패인 수에 따라 리스트 레이아웃이 후보에서
  탈락해 토글이 캔버스에 고착된다.

### 설정 옵션

`auto_layout false` (뷰 상태는 사용자 의도로 유지), `mouse_mode true` (요구사항 1 — 절대 끄지 마라),
`show_release_notes false`.

⚠️ **zellij 0.44.3 은 모르는 최상위 옵션을 조용히 무시한다.** `zellij setup --check` 의
"Well defined" 는 옵션 이름이 맞다는 뜻이 **아니다** (없는 옵션도 통과한다).
옵션은 `zellij setup --dump-config` 에 실제로 있는 것만 써라.

### `override_layout` 은 기본값이 패인을 죽인다

`override_layout` 을 `retain_existing_terminal_panes=false, retain_existing_plugin_panes=false`
(**둘 다 CLI 기본값**)로 호출하면 레이아웃에 맞지 않는 기존 패인이 탭에서 제거된다 —
**호출한 플러그인 자신의 패인까지** 사라진다. 절대원칙 2 위반이다.

→ 쓸 거면 **둘 다 반드시 `true`**. 런타임 실측 결과다.

### 뷰 전환 API 요약 (런타임 4회 왕복 검증 완료)

```rust
replace_pane_with_existing_pane(밀려날_패인, 올라올_패인, /* suppress_replaced_pane */ true)
```
- `true`: 밀려난 패인은 죽지 않고 suppressed 로 이동. 계속 실행되고 `PaneManifest` 에 남는다.
- `false`: **패인이 닫힌다. polycanv 에서 이 값은 등장해서는 안 된다.**
- 포커스 이동·지오메트리 승계가 내부에서 처리되므로 추가 호출이 필요 없다.
- suppressed 를 타일로 되돌리는 것은 swap layout 으로 **불가능**하다. 이 API 또는
  `show_pane_with_id` 가 필요하다.

### 상태 감지는 4개 CLI 전부 ①계층(훅/이벤트)이다

claude code / codex / opencode / qwen code 모두 waiting 과 finished 가 훅·이벤트로 갈린다.
**기본 제공 4종에 출력 패턴매칭을 만들지 마라.** `StatusSource::Pattern` 은 설정으로 추가되는
미지의 도구를 위해 계약에 남아 있는 것이지, 이 4개를 위한 것이 아니다.

codex 훅 설정은 `$CODEX_HOME/config.toml` 안의 **테이블**이다 (파일 경로가 아니다).
이벤트 키는 snake_case(`permission_request`), 페이로드의 `hook_event_name` 은 PascalCase
(`PermissionRequest`)로 온다. 상세·실측 페이로드: `docs/research/cli-status-hooks.md` 9장.

**결정 (2026-08-19, 사용자)**: codex 훅 신뢰 게이트는 **최초 1회 사용자 안내**로 넘는다.
`--dangerously-bypass-hook-trust` 를 polycanv 가 붙이지 마라 — 그건 사용자의 보안 결정을 대신하는 것이다.
polycanv 는 훅이 신뢰되지 않은 상태를 **감지해서 안내**하고, 사용자가 codex TUI 훅 화면에서
한 번 신뢰시키면 `hooks.state` 에 지속된다. 그 뒤로는 자동이다.

→ 런처 구현 시: codex 패인을 띄운 뒤 상태 이벤트가 오지 않으면 "훅 신뢰가 필요하다"고
   사용자에게 알릴 수단이 필요하다. 조용히 ⚪ 로 두면 사용자는 도구가 고장난 줄 안다.

### 플러그인 검증의 한계 — 사람이 직접 봐야 하는 지점

**`dump-screen` 은 플러그인 패인에서 항상 빈 출력이다.** 우리 플러그인만이 아니라 zellij 내장
`tab-bar` / `status-bar` / `strider` 도 전부 1바이트다(실측). **플러그인 렌더링은 외부에서 관찰할 수
없다.** 따라서 렌더 로직은 호스트 단위 테스트로 덮고, 화면 확인은 사람이 붙어서 해야 한다.

관찰 가능한 것은 **부수 효과**뿐이다: `list-clients`(포커스 이동), `list-tabs --all`
(TILED/HIDDEN/SWAP_LAYOUT), `dump-layout`(배치), zellij 로그(플러그인이 `eprintln!` 한 것).

⚠️ **zellij 플러그인은 최초 실행 시 권한 승인이 필요하다.** 승인 전에는 로드는 되지만
**키 입력에 반응하지 않는다.** 승인 대화는 플러그인 패인에 그려지므로 위 한계 때문에 보이지 않는다.
실측: `y` + Enter 를 보낸 뒤에야 숫자키 선택이 동작했다.
→ **제품 관점**: 사용자가 처음 polycanv 를 켜면 사이드바가 "먹통"으로 보인다. 설치 문서에 반드시
   적거나, 첫 실행 안내를 넣어야 한다.

### 사이드바 wasm 설치 위치 (실측)

`~/.config/zellij/plugins/polycanv-sidebar.wasm` 에서 로드된다.
레이아웃에서 `plugin location="file:~/.config/zellij/plugins/polycanv-sidebar.wasm"` 로 참조하며
**`~` 는 확장된다**(실측). 설치 스크립트가 이 경로에 놓아야 한다.

개발 중 재빌드 후에는 이 경로로 다시 복사해야 반영된다:
```
cargo build --release --target wasm32-wasip1   # plugins/sidebar 에서
cp target/wasm32-wasip1/release/polycanv-sidebar.wasm ~/.config/zellij/plugins/
```

### 뷰 전환은 **파이프**로 몰아야 한다 (키바인딩 → 플러그인)

로드맵 ④ 검증 중 드러난 함정 두 가지. 둘 다 실측이다.

**1. `NextSwapLayout` 을 키에 직접 걸면 안 된다.** 그건 2단계 전환의 **2단계만** 한다.
패인을 접고 펼치는 1단계가 빠지므로, 리스트로 가도 메인 영역이 N등분될 뿐이고
캔버스로 돌아와도 접힌 패인이 되살아나지 않는다.

**2. 플러그인 로컬 키('v'/Tab)로만 두면 안 된다.** 접기·선택 직후 포커스는 **메인 터미널로
옮겨간다**(그게 정상 동작이다 — 바로 입력 가능해야 하므로). 그 상태에서 플러그인 키를 누르면
그냥 터미널에 글자가 타이핑되고, **사용자는 캔버스로 돌아올 방법을 잃는다.**

→ **정답: 전역 키바인딩이 플러그인에 파이프 메시지를 보낸다.**
```kdl
bind "Ctrl y" {
    MessagePlugin "file:~/.config/zellij/plugins/polycanv-sidebar.wasm" {
        name "toggle_view"
    }
}
```
플러그인이 받는 파이프 이름: `toggle_view` / `view_list` / `view_canvas` / `polycanv:state`(상태 이벤트).

**실측 (포커스가 사이드바가 아닌 터미널에 있는 상태에서):**
```
$ zellij -s <세션> pipe --name toggle_view --plugin file:~/.config/zellij/plugins/polycanv-sidebar.wasm
TILED=5 SWAP=BASE  →  TILED=2 SWAP=list  →  (한 번 더) TILED=5 SWAP=BASE
프로세스 13개 전 구간 동일
```
접기·펼치기 왕복이 포커스와 무관하게 동작한다. **이것이 뷰 전환의 정식 경로다.**

⚠️ 앞서 "`zellij setup --check` 의 Well defined 를 믿지 마라"고 적었는데 **범위를 정정한다.**
그 한계는 **최상위 옵션**에 한정된다. 키바인딩의 **액션 이름은 제대로 검증된다**
(실측: `ThisActionDoesNotExist` → `Unsupported action` 오류). 키바인딩 문법은 `--check` 로 확인해도 된다.

### 상태 훅 브리지 — `zellij pipe` 는 **절대 기다리면 안 된다**

`scripts/polycanv-hook.sh` 가 CLI 훅 JSON(stdin)을 받아 사이드바의 `polycanv:state` 파이프로
[`StatusEvent`] 를 보낸다. 훅 기반 CLI(claude / codex / qwen)는 **상태 감지 플러그인 없이
이 스크립트만으로 신호등이 켜진다.** 패인 식별은 zellij 가 주입하는 `ZELLIJ_PANE_ID` 로 한다(실측).

⚠️ **플러그인이 `ReadCliPipes` 권한을 요청하지 않으면 `zellij pipe` 가 영원히 반환하지 않는다.**

페이로드를 실은 CLI 파이프는 플러그인이 `unblock_cli_pipe_input()` 을 불러야 닫힌다.
그런데 그 호출에는 **`PermissionType::ReadCliPipes` 가 필요하고, 없으면 조용히 거부된다** —
플러그인 쪽에는 아무 신호도 없고 zellij 로그에만 한 줄 남는다:

```
permission 'ReadCliPipes' denied - Command 'UnblockCliPipeInput' denied
```

→ **파이프를 받는 플러그인은 반드시 세 권한을 함께 요청한다:**
`ReadApplicationState` / `ChangeApplicationState` / **`ReadCliPipes`**.
실측: 권한 추가 전 수 분간 매달림 → 추가 후 **0초 반환**.

부수적으로: 페이로드는 **인자로** 넘겨라. STDIN 으로 주면 `zellij pipe` 가 스트리밍 모드
(`tail -f` 용도)로 들어간다.

그래도 훅 스크립트는 백그라운드 + 감시 프로세스로 던진다. 원인은 해결됐지만, 훅이 매달리면
**CLI 의 턴이 통째로 멈춘다.** 신호등 하나 때문에 사용자의 작업을 세우는 위험은 방어를
남겨둘 만하다.

**와이어 포맷은 계약 테스트로 고정돼 있다** (`crates/protocol/src/event.rs` 의
`훅_브리지가_보내는_와이어_포맷을_그대로_읽는다`). 셸과 Rust 가 어긋나면 신호등이 조용히 안 켜질
뿐 아무 오류도 안 나므로, 스크립트 출력을 바꾸면 **그 테스트부터 깨져야 한다.**

### ⚠️ 이 저장소에서 여러 세션이 동시에 작업 중이다

루트 `Cargo.toml` 의 `members` 가 서로 덮어써지는 일이 실제로 발생했다.
`members` 를 고칠 때는 **먼저 현재 값을 읽고 병합**해라. 통째로 쓰지 마라.

### 훅은 사용자 설정을 고치지 않고 얹는다 — `--settings` (실측)

claude code 는 `--settings <파일>` 로 **설정을 얹어서** 실행할 수 있다. 실제 인증은 그대로 쓰면서
훅만 추가되므로, **polycanv 가 사용자의 `~/.claude/settings.json` 을 건드릴 필요가 없다.**

→ **런처는 claude 패인을 띄울 때 polycanv 가 생성한 설정 파일을 `--settings` 로 넘겨라.**
   사용자 설정을 수정하는 설계는 채택하지 마라 — 되돌리기 어렵고, 다른 도구와 충돌하며,
   사용자가 polycanv 를 지워도 흔적이 남는다.

codex 에는 같은 수단으로 `CODEX_HOME` 이 있다(실측: 격리된 CODEX_HOME + auth.json 심볼릭 링크로
훅 검증을 마쳤다). 다만 codex 는 훅 신뢰 게이트가 있어 최초 1회 사용자 승인이 필요하다.

### 상태 감지 — 실제 claude 턴으로 끝까지 확인했다

zellij 패인 안에서 실제 인증된 claude 턴을 돌린 결과:
```
UserPromptSubmit (pane=21) → {"pane":{"terminal":21},"state":"running","source":"hook",...}
Stop             (pane=21) → {"pane":{"terminal":21},"state":"finished","source":"hook",...}
```
패인 식별 → 상태 매핑 → 와이어 포맷이 실제 데이터로 맞물린다. 🔴 까지 도달한다.
**남은 것은 사이드바가 그것을 화면에 어떻게 그리는지뿐이고, 그건 외부에서 볼 수 없다.**

### ★ 파이프의 진짜 제약 — 승인되지 않은 인스턴스에 보내면 호출자가 매달린다

세 번의 실측 끝에 정리된 결론이다. 앞선 "브로드캐스트가 문제" / "설정 불일치가 문제" 진단은
**증상이었고, 원인은 이것 하나다:**

1. `zellij pipe` 는 **일치하는 인스턴스가 없으면 새로 띄운다** (도움말 명시).
   인스턴스 일치는 **URL + 설정** 으로 판단한다 (`--plugin-configuration` 도움말 명시:
   *"the same plugin with different configuration is considered a different plugin"*).
2. **새로 뜬 인스턴스는 권한 미승인 상태다.** 승인 전에는 `unblock_cli_pipe_input` 이 거부된다.
3. → 파이프가 닫히지 않고 **호출자가 영구히 매달린다.**

실측으로 성공했던 파이프는 전부 **직전에 `y` 로 승인한 인스턴스**를 향한 것이었다.

**따라서:**
- **플러그인은 레이아웃에 마운트해서 최초 1회 승인받은 인스턴스를 쓴다.** 파이프가 인스턴스를
  새로 띄우게 두지 마라.
- **키바인딩(`MessagePlugin`)이 스크립트 파이프보다 안전하다.** `PipeSource::Keybind` 는
  CLI 파이프가 아니라 unblock 대상이 아니다.
- **스크립트에서 파이프를 쓸 때는 반드시 타임아웃을 건다.** `scripts/polycanv-hook.sh` 가 그렇게 돼 있다.
- 파이프를 받는 플러그인은 `ReadCliPipes` 를 요청한다(필요조건이지 충분조건이 아니다 — 승인이 있어야 한다).

### 런처 v1 과제 — 도구 목록은 **인스턴스 설정이 아니라 파일**에서 읽어야 한다

현재 런처는 도구 목록을 **플러그인 인스턴스의 configuration** 에서 읽는다. 이게 실측에서 문제를 냈다:

`zellij pipe --plugin <url>` 은 **URL 로만 대상을 고른다.** 같은 wasm 의 인스턴스가 여러 개 있으면
(설정이 있는 것 / 없는 것) **엉뚱한 인스턴스가 응답**한다. 실측 로그:
```
polycanv-launcher: 'qwen' 라는 도구가 설정에 없다. ... 현재 등록된 도구: []
```
→ 파이프로 띄우려던 도구가 조용히 안 떴다. (안내 메시지를 넣어둔 덕에 원인이 바로 드러났다.)

**v1 에서는 도구 목록을 공용 파일(`config/tools.kdl` 등)에서 읽어라.** 이유가 둘이다:
어느 인스턴스가 응답하든 같은 목록을 보고, **파이프 호출자가 설정 문자열을 몰라도 된다**
(설정이 인스턴스 정체성의 일부라 호출자가 토씨 하나까지 맞춰야 하는 지금 구조는 깨지기 쉽다).

### 브로드캐스트 파이프(`--plugin` 생략)를 쓰지 마라

`zellij pipe` 에서 `--plugin` 을 빼면 **듣고 있는 모든 플러그인**에 간다. 그중 하나라도
`ReadCliPipes` 권한이 없어 unblock 하지 못하면 **파이프 전체가 매달린다** (실측: 5분 초과).
오래된 인스턴스가 남아 있기만 해도 걸린다. **항상 대상을 명시해라.**

### 요구사항 2 — 4개 CLI 동시 구동 실측 완료

claude / codex / opencode / qwen 이 **동시에** zellij 패인에서 도는 것을 프로세스로 확인했다
(`ps` 로 zellij 서버의 자식 검사). qwen 은 `npm i -g @qwen-code/qwen-code` (89MB).

### 권한 부족은 우아하게 실패하지 않는다 — **플러그인이 죽는다**

`open_command_pane` 은 `PermissionType::RunCommands` 를 요구한다. 빠뜨리면 호스트가 거부하고
**플러그인이 wasm `unreachable` 로 패닉한다.** 실측 로그:
```
permission 'RunCommands' denied
thread 'main' panicked at plugins/launcher/src/plugin.rs:18:1: wasm `unreachable` instruction executed
Failed to apply event to plugin 19
```
증상은 "키를 눌러도 아무 일이 없다" 였고, 원인은 플러그인이 매 이벤트마다 죽고 있던 것이었다.

**실패 방식이 둘이다** (둘 다 실측):
- 값을 반환하지 않는 호출: `permission '...' denied` 로그 + wasm `unreachable` 패닉
- **값을 반환하는 호출**(`open_command_pane` 등): 호스트가 응답을 쓰지 않아
  `failed to deserialize bytes from stdin / EOF while parsing a value` 로 `zellij-tile` shim 안에서 패닉

두 번째가 특히 헷갈린다 — **오류 메시지가 권한을 전혀 언급하지 않는다.** 직렬화 버그처럼 보인다.
플러그인이 "키를 눌러도 아무 일이 없다" 로 보이면 **먼저 권한 승인 여부를 의심해라.**
권한은 **wasm url + configuration 조합마다** 따로 승인받아야 한다 —
설정을 바꾸면 승인도 다시 받아야 한다.

→ **새 zellij API 를 쓸 때는 필요한 permission 을 먼저 확인해라.** 지금까지 필요했던 것:
`ReadApplicationState`(패인 조회) / `ChangeApplicationState`(배치 변경) /
`RunCommands`(패인에서 명령 실행) / `ReadCliPipes`(CLI 파이프 unblock).

### 런처는 `LaunchOrFocusPlugin` 으로 연다 (실측)

파이프로 런처를 띄우지 마라 — 파이프는 일치하는 인스턴스가 없으면 새로 띄우고, 새 인스턴스는
권한 미승인이라 호출자가 매달린다. `LaunchOrFocusPlugin` 은 **이미 떠 있으면 포커스만 옮긴다**
(실측: 두 번 호출해도 같은 `plugin_19`). 승인된 인스턴스 하나를 계속 재사용한다.

도구 목록은 `config/keybinds.kdl` 의 그 블록에 있다. **항목 추가·수정은 그 블록만 고치면 된다.**
실측: `tool_probe3 "echo launcher-key-works"` 를 넣고 키를 누르니 그 명령의 패인이 생성됐다.

### polycanv 는 CLI 의 인증을 요구하지 않는다

하는 일은 **터미널을 띄우고 훅을 읽는 것**이다. 그 CLI 에 로그인했는지는 사용자 사정이지
polycanv 의 의존성이 아니다. 이 구분 덕분에 인증 없이도 훅 경로를 검증할 수 있다
(실측: qwen 을 프로바이더 미연결 상태로 띄워 `SessionStart` → 브리지 → 🔘 까지 확인).

→ **설계에 반영**: 신호등이 안 켜지는 것과 CLI 가 인증되지 않은 것은 다른 문제다.
   전자는 훅 배선 문제이고, 후자는 사용자가 해결할 일이다. 오류 안내에서 둘을 섞지 마라.

### 요구사항 3 — 셸은 polycanv 에게 "그냥 실행할 명령"이다

PowerShell 을 macOS 에 설치해 실측했다 (pwsh 7.6.5): polycanv 패인에서 실행되고 입력에 반응한다.
**polycanv 가 셸에 대해 하는 일은 런처가 명령을 띄우는 것뿐**이므로, Windows 의 pwsh 나
WSL(`wsl.exe`/배포판 셸)도 **같은 코드 경로**를 탄다 — 도구 목록에 한 줄 추가하면 끝이다.

→ WSL 자체는 Windows 에서 확인해야 하지만, **그건 polycanv 의 미구현이 아니라 플랫폼 확인**이다.
   이 둘을 구분해서 보고해라.

실측: claude / codex / opencode / qwen / pwsh **5개 동시 구동** 확인.

### 패인 메타데이터 조회는 틱당 상수 개로 묶어라 (실측 버그)

`get_pane_cwd` / `get_pane_running_command` 는 **호스트 왕복이고 zellij 쪽 타임아웃이 100ms** 다
(`zellij-server/src/plugins/zellij_exports.rs`). 사이드바가 타이머마다 **모든 패인**을 훑으면
틱당 2N 번의 블로킹 호출이 되고, 패인이 몇십 개만 돼도 전부 무너진다.

**실측**: 패인 30여 개 세션에서
```
GetPaneRunningCommand timed out  44,720회
GetPaneCwd timed out              4,942회
초당 약 5회 계속 발생
```

증상이 고약하다 — **오류가 사용자에게 보이지 않는다.** 조회가 실패하면 아이템의
**CLI 종류와 작업 디렉터리가 그냥 빈 채로** 남는다. "사이드바가 좀 허전하네" 로 보일 뿐이다.

→ **[`refresh_targets`]** 가 틱당 대상을 **최대 2개**로 묶는다: 포커스된 패인(사용자가 `cd` 하는 곳)
   \+ 회전 커서 하나(나머지도 결국 갱신된다). 새 패인은 `PaneUpdate` 에서 즉시 한 번만 조회한다.
   선택 로직은 호스트 테스트로 고정돼 있다(틱당 ≤2, 포커스 항상 포함, 회전이 전부를 돈다).

⚠️ **주의**: 이 수정의 런타임 효과는 **깨끗한 세션에서 재측정해야 한다.** 개발 중 띄워둔 옛
플러그인 인스턴스가 남아 있으면 그것들이 옛 코드로 계속 타임아웃을 낸다.
`zellij action start-or-reload-plugin <url>` 또는 새 세션으로 확인해라.

### 플러그인 화면을 들여다보는 법 — `debug_render`

`dump-screen` 은 플러그인 패인에서 항상 빈 출력이다(내장 플러그인 포함). 하지만 플러그인의
**`eprintln!` 은 zellij 로그로 나간다.** 사이드바에 `debug_render "true"` 설정을 주면 그린 프레임을
`[frame:NN] ...` 형태로 로그에 남긴다 — 무엇을 그렸는지 밖에서 읽을 수 있다.

```
zellij action new-pane --plugin file:~/.config/zellij/plugins/polycanv-sidebar.wasm \
  --configuration "debug_render=true"
```
그 다음 zellij 로그에서 `[frame` 을 grep 한다. **화면의 겉모습(깜빡임·색)은 여전히 사람이 봐야
하지만, 내용(항목·신호등 문자·하이라이트 위치)은 이걸로 검증할 수 있다.**

### 헤드리스에서 zellij 를 띄우는 법 — PTY 직접 붙이기

zellij 는 TTY 를 요구하지만, **PTY 를 직접 붙이면 헤드리스에서도 깨끗한 세션을 띄울 수 있다.**
이게 열리면서 "사람이 봐야만 확인 가능"하다고 여겼던 항목들이 대부분 검증 가능해졌다.

```python
master, slave = pty.openpty()
fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))  # ★ exec 전에
pid = os.fork()
if pid == 0:
    os.setsid(); fcntl.ioctl(slave, termios.TIOCSCTTY, 0)
    for t in (0,1,2): os.dup2(slave, t)
    os.execvp(...)
```
- **창 크기는 exec 전에 잡아야 한다.** 나중에 바꾸면 zellij 가 이미 읽은 뒤라 반영되지 않는다
  (실측: VP_COLS 가 80 에 고정). 사이드바는 전체 폭의 22% 라 좁으면 내용이 잘려 검증이 무의미해진다.
- `script -q /dev/null` 도 되지만 stdin 이 소켓이면 `tcgetattr` 로 실패한다. PTY 직접 붙이는 쪽이 안정적이다.
- 부모는 master 를 계속 읽어 비워야 자식이 멈추지 않는다.

### 실측으로 확인한 화면 동작

`debug_render "true"` + PTY 세션으로 아래를 전부 확인했다 (요구사항 4·5의 화면 부분):

```
─ 캔버스 4 ─────────────────────────────    (cols=42)
[반전]1 ⚪ Pane #1  zsh  …/ddul/python/polycanv[/반전]   ← 선택 하이라이트
2▸⚪ Pane #2  zsh  …/ddul/python/polycanv                ← ▸ 는 메인/포커스
```
- **아이템 형식이 사양대로다**: 번호 / 포커스 표식 / 신호등 / 이름 / CLI 종류 / 작업 디렉터리
- **폭 적응**: 좁으면 cwd 를 `…/` 로 중간 생략, 넓어지면(리스트 뷰 58칸) 전체 경로를 보여준다
- **🔴 점등**: `polycanv:state` 로 finished 를 보내면 그 줄이 🔴 로 바뀐다
- **확인 시 해제**: 그 터미널을 실제로 포커스하면 🔴 → ⚪, `▸` 도 따라 이동한다
- **뷰 전환**: 헤더가 `캔버스` → `리스트`, 배치는 TILED 6 → 3, SWAP=list
- **flicker 없음**: 변화 없는 20초 동안 **재렌더 0회** (타이머는 10번 돌았다)

남은 시각 항목은 **색·깜빡임의 실제 느낌**뿐이고, 그건 사람이 봐야 한다.

## 개념의 출발점 — cate

**[0-AI-UG/cate](https://github.com/0-AI-UG/cate)** (MIT, TypeScript/Electron).
*"An infinite canvas IDE for parallel coding agents."*

polycanv 의 **컨셉은 여기서 왔다.** 기술 기반은 zellij 지만, "왜 이런 물건이 필요한가"에 대한
답은 cate 가 먼저 내놨다.

### cate 가 푼 문제

> 캔버스가 mission control 이 된다. 터미널마다 에이전트가 **작업 중인지 · 끝났는지 ·
> 사용자를 기다리는지** 보이고, 입력이 필요해지는 순간 알린다. 한 번 클릭하면 git worktree 가
> **캔버스 위에 자기 색깔의 영역**을 갖는다 — 다섯 에이전트가 다섯 브랜치에서 돌면
> **탭 더미가 아니라 눈에 보이게 분리된 다섯 개의 작업 흐름**이 된다.

핵심은 **공간이다.** 세션에 위치가 있으면 사람이 그 위치를 기억한다. 탭 목록에서는 그게 안 된다.

### polycanv 가 가져온 것

- **에이전트 상태를 터미널에 붙여 보여준다** — running / waiting / finished.
  cate 의 상태 감지 프로토콜(turn start / turn end / permission prompt)이 그대로 우리 계약이 됐다.
- **여러 에이전트를 병렬로 감시한다**는 전제. 하나씩 보는 도구가 아니다.
- **접혀 있어도 상태는 보인다** — 안 보이는 세션이 끝나도 놓치지 않는다.

### 의도적으로 다르게 한 것

| | cate | polycanv |
|---|---|---|
| 실행 형태 | Electron 데스크톱 앱 | **TUI** |
| 이유 | — | 설치 마찰·사내 proxy/인증서 리스크 최소, 경량 |
| 배치 | 무한 줌 캔버스 (자유 좌표) | zellij 타일 + swap layout |
| 편집기·브라우저 | 포함 | **없음** — 터미널만 |

### 아직 못 따라간 것

cate 의 **"worktree 마다 색깔 있는 영역"** 이 우리에게 없다. 우리 캔버스는 그냥 타일이라
세션들이 **공간적 정체성**을 갖지 못한다. 사용자가 말한 "세션이 어디 있는지 관리"의 핵심이
바로 그건데, 지금은 순서만 있고 영역이 없다. → 이슈로 남겼다.

**cate 를 베끼지 않는다.** 스택도 범위도 다르다. 다만 **개념의 출처를 숨기지 않는다** —
어디서 왔는지 아는 편이 왜 그렇게 만들었는지 이해하는 데 낫다.
