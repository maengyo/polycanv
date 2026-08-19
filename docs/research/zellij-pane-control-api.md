# Zellij 플러그인 패인 제어 API 조사

**조사 대상 리스크**: [높음] 리스크 1 — "좌측 사이드바에서 항목 선택 → 그 패인이 우측 메인 영역으로 교체되어 나타난다"를 WASM 플러그인이 프로그래밍 방식으로 구현 가능한가.

**조사일**: 2026-08-19 (1차 정적 판독) / 2026-08-19 (2차 런타임 스파이크)
**기준 버전**: `zellij` / `zellij-tile` / `zellij-utils` / `zellij-server` **0.44.3**
 - crates.io sparse index 기준 최신 published 버전 (yanked 아님)
 - GitHub 최신 릴리스 `v0.44.3`, published `2026-05-13T07:49:54Z`
 - 런타임 검증 환경: `/opt/homebrew/bin/zellij` **0.44.3**, `rustc`/`cargo` **1.97.1**,
   wasm 타깃 **`wasm32-wasip1`**, macOS (Darwin 24.6.0)

## 이 문서의 근거 등급 표기

각 주장 앞에 근거 등급을 붙였다. **섞어 읽지 마라.**

| 표기 | 뜻 |
|---|---|
| **[런타임]** | 실제로 zellij 0.44.3을 띄우고 플러그인을 실행해 확인함. 명령과 출력이 아래에 있다 |
| **[정적]** | 0.44.3 소스 판독만 했고 실행으로는 확인하지 않음 |
| **[추정]** | 근거는 있으나 확인하지 않음 |

**조사 방법**
1. **1차(정적)**: 로컬에 zellij·cargo·rustc가 없어(`command not found`) crates.io에서 `.crate` tarball을
   직접 받아 소스를 읽었다.
2. **2차(런타임)**: 툴체인이 설치된 뒤, 폐기용 테스트 플러그인(`spike`)을 실제로 빌드해
   전용 세션 `polycanv-spike` 에서 실행하고 검증했다. 관측은 플러그인 내부 `PaneManifest` 덤프와
   외부 `zellij action list-panes` / `dump-screen -f` / `ps` 를 **교차**해서 했다.

---

## 결론

### 1. [런타임] 직접 가능하다. swap layout 우회는 필요 없다. **실제로 돌려서 확인했다.**

`zellij-tile` 0.44.3은 **`replace_pane_with_existing_pane`** 를 노출한다. 이 함수 **단 한 번의 호출**로
"메인 슬롯의 패인 A를 죽이지 않고 숨긴 뒤, 그 자리에 기존에 살아있던 패인 B를 끼워 넣는" 동작이 완성된다.

```rust
// zellij-tile-0.44.3/src/shim.rs:2711
pub fn replace_pane_with_existing_pane(
    pane_id_to_replace: PaneId,
    existing_pane_id: PaneId,
    suppress_replaced_pane: bool,
)
```

`suppress_replaced_pane: true` 로 호출하면 밀려난 패인 A는 **kill 되지 않고 `suppressed_panes` 로 이동**한다.

**[런타임] 왕복 2회(총 4번 교체) 테스트 결과 — 근거 F 참조:**

| 검증 항목 | 결과 |
|---|---|
| (a) 두 프로세스가 모두 살아있는가 | **PASS** — 두 bash PID가 4회 교체 내내 **동일**하게 유지 |
| (b) 밀려난 패인의 스크롤백이 보존되는가 | **PASS** — suppressed 상태에서도 60줄 + 마커 전부 보존 |
| (c) 메인 슬롯의 크기·위치가 유지되는가 | **조건부 PASS** — `auto_layout false` 필요. 아래 3번 참조 |
| suppressed 패인 회수 경로 (upstream `// TODO: test this`) | **PASS** — 왕복의 복귀 단계가 정확히 이 경로이며 4회 모두 성공 |

suppressed 패인은 계속 PTY 출력을 받아 자기 터미널 버퍼를 갱신하며, `PaneManifest` 에도
`is_suppressed: true` 로 계속 노출된다 **[런타임 확인]**.
즉 CLAUDE.md 절대원칙 2(뷰 전환은 프로세스를 죽이지 않는다)·4(리스트 뷰는 끄는 게 아니라 접는 것)를
**정확히 만족한다**.

**[정적]** `false` 로 호출하면 `close_pane_and_replace_with_other_pane` 경로를 타서 A가 **닫힌다**.
(이 분기는 파괴적이라 런타임으로 시험하지 않았다.) polycanv에서는 **항상 `true`** 를 써야 한다.

### 2. [런타임+정적] 필요 permission은 `ChangeApplicationState` 하나 (+ 조회용 `ReadApplicationState`)

**[정적]** 패인 제어 계열(포커스/이동/리사이즈/스택/플로팅/교체/swap layout/override layout)이
**전부 동일하게** `PermissionType::ChangeApplicationState` 로 묶여 있다. 패인 목록·정보 조회는
`ReadApplicationState`. polycanv 플러그인 3종은 이 두 개만 요청하면 패인 제어 전부가 열린다.

**[런타임]** 이 두 개만 요청한 플러그인이 실제로 `replace_pane_with_existing_pane` 과
`override_layout` 을 모두 성공적으로 호출했다. 로그: `SPIKE: permission result = Granted`.

### 3. [런타임] `auto_layout` 은 **반드시 꺼야 한다**

이번 스파이크에서 새로 확인된, 문서화되지 않은 제약이다. 같은 교체 시퀀스를 두 설정에서 각각 돌렸다.

| 설정 | 사이드바(플러그인) 패인 | 결과 |
|---|---|---|
| `auto_layout true` | `rows=3` → **`rows=1` 로 찌그러짐**, 탭 전체가 swap layout으로 재배치됨 | **사용 불가** |
| `auto_layout false` | `x=0 y=0 rows=3 cols=200` **완전 유지** (4회 교체 내내) | **정상** |

원인은 정적 판독으로 이미 예고했던 지점이다 — `Tab::extract_pane` 의 tiled 분기
(`tab/mod.rs:4114-4119`)가 `if self.auto_layout && !is_tiled_damaged { relayout_tiled_panes(false) }`
로 **자동 재배치를 트리거**한다. 사이드바에서 고른 패인이 현재 화면에 보이는 tiled 패인일 때 이 경로를 탄다.

→ **polycanv는 `auto_layout false` 를 기본 config로 강제해야 한다.**

### 4. swap layout은 패인을 죽이지 않으며, 슬롯 배정을 레이아웃 쪽에서 제어할 수 있다 [정적]

- **죽이지 않는다**: `next_swap_layout()`/`previous_swap_layout()` → `Tab::relayout_tiled_panes` →
  `LayoutApplier::apply_tiled_panes_layout_to_existing_panes`. 함수명 그대로 **기존 패인 객체를 재배치**할 뿐,
  spawn/kill이 없다.
- **슬롯 제어 가능**: 배정은 3단계 우선순위로 이뤄진다.
  1. **`layout.run` 완전 일치** — KDL 노드의 `command "..."` / `plugin location="..."` 이 기존 패인의
     `invoked_with()` 와 같으면 그 패인을 그 슬롯에 넣는다. ← **여기서 제어한다**
  2. **동일 `logical_position`**
  3. 남은 패인을 logical_position 순으로 best-effort 배정, 그래도 남으면 tiled_panes가 알아서 자리 배치
- **단, 한계 있음**: swap layout은 `self.tiled_panes.drain()` 만 대상으로 한다. **suppressed 패인은
  swap layout이 건드리지 않는다.** 따라서 "숨겨둔 터미널을 레이아웃 전환만으로 메인에 끌어올리는" 것은
  swap layout으로 **불가능**하다 — 그건 1번의 `replace_pane_with_existing_pane` 몫이다.

### 5. 우회안은 불필요 (직접 API가 런타임 검증까지 끝났으므로)

**[정적]** 굳이 백업안을 꼽자면 **swap layout이 아니라** `hide_pane_with_id` + `focus_pane_with_id`
조합이다. 구체적 호출 순서는 「설계에 미치는 영향」의 대안 B 참조.
swap layout 단독 우회는 위 4번 한계 때문에 **성립하지 않는다**.
직접 API가 4회 왕복 검증을 통과했으므로 **이 우회안은 구현하지 마라.**

### 6. [런타임] `override_layout` 은 `retain_*` 플래그를 **반드시 `true`** 로 줘야 한다

`retain_existing_terminal_panes=false, retain_existing_plugin_panes=false` (**둘 다 CLI 기본값**)로
호출하면 레이아웃에 맞지 않는 기존 패인이 **탭에서 제거된다 — 호출한 플러그인 자신의 패인까지**
(`Bye from plugin 3` 로그). 절대원칙 2 위반이다. 둘 다 `true` 면 모든 기존 패인이 보존된다.
근거 F-4 참조.

---

## 근거

### 실행한 명령과 출력 (1차 — 소스 확보)

1차 조사 시점에는 툴체인이 없었다:
```
$ zellij --version
zsh: command not found: zellij
$ cargo --version
zsh: command not found: cargo
$ rustc --version
zsh: command not found: rustc
```
→ 당시엔 로컬 검증 불가. 소스 직접 확보로 전환.

2차(런타임) 시점에는 설치되어 있었다:
```
$ /opt/homebrew/bin/zellij --version
zellij 0.44.3
$ rustc --version
rustc 1.97.1 (8bab26f4f 2026-07-14)
$ cargo --version
cargo 1.97.1 (c980f4866 2026-06-30)
$ rustup target list --installed
aarch64-apple-darwin
wasm32-wasip1
```
→ 근거 F로 이어진다.

```
$ curl -A "polycanv-research" https://index.crates.io/ze/ll/zellij-tile
# 마지막 줄: {"vers":"0.44.3", "yanked":false}
$ curl -A "polycanv-research" https://index.crates.io/ze/ll/zellij
# 마지막 줄: {"vers":"0.44.3", "yanked":false}

$ curl -A "..." -L https://static.crates.io/crates/zellij-tile/zellij-tile-0.44.3.crate   # 42,164 bytes
$ curl -A "..." -L https://static.crates.io/crates/zellij-utils/zellij-utils-0.44.3.crate # 6,877,911 bytes
$ curl -A "..." -L https://static.crates.io/crates/zellij-server/zellij-server-0.44.3.crate # 699,522 bytes
$ tar xzf ...

$ curl https://api.github.com/repos/zellij-org/zellij/releases/latest
tag: v0.44.3
name: Release v0.44.3
published: 2026-05-13T07:49:54Z
```

아래 파일 경로는 위 tarball을 푼 기준(`<crate>-0.44.3/...`)이며,
GitHub `zellij-org/zellij` tag `v0.44.3` 의 동일 경로와 대응한다.

### 근거 A — 핵심 호출 경로 전체 추적

`replace_pane_with_existing_pane` 이 실제로 "죽이지 않고 교체"하는지, 그리고 **suppressed 상태의 패인을
다시 꺼내올 수 있는지**를 끝까지 따라갔다. 이게 이 조사의 핵심이다.

**1) 플러그인 측 진입점**
```
zellij-tile-0.44.3/src/shim.rs:2711
  pub fn replace_pane_with_existing_pane(pane_id_to_replace, existing_pane_id, suppress_replaced_pane)
    → PluginCommand::ReplacePaneWithExistingPane(PaneId, PaneId, bool)
```
커맨드 정의: `zellij-utils-0.44.3/src/data.rs:3546`

**2) 서버 측 수신 → Screen 으로 전달**
```
zellij-server-0.44.3/src/plugins/zellij_exports.rs:690   (커맨드 디스패치)
zellij-server-0.44.3/src/plugins/zellij_exports.rs:5174  (ScreenInstruction 전송)
zellij-server-0.44.3/src/screen.rs:9288                  (인스트럭션 핸들러)
```

**3) `Screen::replace_pane_with_existing_pane`** — `zellij-server-0.44.3/src/screen.rs:4408`

핵심 3줄:
```rust
// 교체 대상 A 가 속한 탭 찾기 — has_pane_with_pid 는 suppressed 도 포함
.find(|(_tab_index, tab)| tab.has_pane_with_pid(&pane_id_to_replace))
// 투입할 B 를 그 탭에서 꺼냄 — 두 번째 인자 dont_swap_if_suppressed = true
.and_then(|(_, t)| t.extract_pane(pane_id_of_existing_pane, true))
// suppress_replaced_pane 분기
if suppress_replaced_pane {
    tab.1.suppress_pane_and_replace_with_other_pane(pane_id_to_replace, extracted_pane, None);
} else {
    tab.1.close_pane_and_replace_with_other_pane(pane_id_to_replace, extracted_pane, None);
}
```

**4) `Tab::has_pane_with_pid` 가 suppressed 를 포함하는가 → YES**
`zellij-server-0.44.3/src/tab/mod.rs:2675`
```rust
pub fn has_pane_with_pid(&self, pid: &PaneId) -> bool {
    self.tiled_panes.panes_contain(pid)
        || self.floating_panes.panes_contain(pid)
        || self.suppressed_panes.values().any(|s_p| s_p.1.pid() == *pid)
}
```
(대조군으로 `has_non_suppressed_pane_with_pid` 가 :2683 에 따로 존재 — 즉 suppressed 포함은 의도된 설계다.)

**5) `Tab::extract_pane(id, dont_swap_if_suppressed = true)` 가 suppressed 패인을 꺼내오는가 → YES**
`zellij-server-0.44.3/src/tab/mod.rs:4061`

`dont_swap_if_suppressed = true` 이므로 첫 분기(:4066, 스크롤백 에디터용)는 **건너뛴다**. 이후:
- floating 에 있나? (:4084) → 아니오
- tiled 에 있나? (:4107) → 아니오
- **suppressed 에 있나? (:4125-4133) → 예, 여기서 꺼낸다**
```rust
} else if let Some(suppressed_key_of_pane) = self
    .suppressed_panes
    .iter()
    .find_map(|(key, (_, pane))| if &pane.pid() == &id { Some(*key) } else { None })
{
    // TODO: test this (from the path in screen.rs focus_plugin_pane ~line 2519
    self.suppressed_panes.remove(&suppressed_key_of_pane).map(|s_p| s_p.1)
}
```
→ **suppressed 패인을 살아있는 채로 회수한다.** 이것이 "왕복 전환"이 성립하는 근거다.
⚠️ 단 상단에 upstream 저자가 남긴 `// TODO: test this` 주석이 있다 — 상류에서 **테스트되지 않았다고 명시한
경로**다. (→ 「미해결」)

**6) `Tab::suppress_pane_and_replace_with_other_pane`** — `zellij-server-0.44.3/src/tab/mod.rs:2330`
```rust
let mut replaced_pane = if self.floating_panes.panes_contain(&pane_id_to_replace) {
    self.floating_panes.replace_pane(pane_id_to_replace, pane_to_replace_with).ok()
} else {
    self.tiled_panes.replace_pane(pane_id_to_replace, pane_to_replace_with)
};
if let Some(replaced_pane) = replaced_pane.take() {
    let is_scrollback_editor = false;
    self.insert_suppressed_pane(replaced_pane.pid(), (is_scrollback_editor, replaced_pane));
}
```
→ 밀려난 A는 **kill 없이** `suppressed_panes` 에 보관. 키는 A 자신의 pid.

**7) `TiledPanes::replace_pane` — 지오메트리와 포커스가 승계되는가 → YES**
`zellij-server-0.44.3/src/panes/tiled_panes/mod.rs:145`
```rust
let removed_pane_geom = removed_pane.position_and_size();
let removed_pane_geom_override = removed_pane.geom_override();
with_pane.set_geom(removed_pane_geom);          // ← B가 A의 자리·크기를 그대로 인수
...
self.move_clients_between_panes(pane_id, with_pane_id);  // ← 포커스도 B로 자동 이동
```
→ **별도의 `focus_pane_with_id` 호출이 불필요하다.** 메인 슬롯 좌표를 플러그인이 계산할 필요도 없다.

**8) suppressed 패인이 진짜로 "계속 실행"되는가 → YES**
`zellij-server-0.44.3/src/tab/mod.rs:2686` `handle_pty_bytes`
```rust
self.tiled_panes.get_pane_mut(PaneId::Terminal(pid))
    .or_else(|| self.floating_panes.get_pane_mut(PaneId::Terminal(pid)))
    .or_else(|| self.suppressed_panes.values_mut()
        .find(|s_p| s_p.1.pid() == PaneId::Terminal(pid)).map(|s_p| &mut s_p.1))
```
→ 화면에 없어도 PTY 바이트를 받아 터미널 버퍼를 갱신한다. `hold_pane`(:2720 부근)도 동일 패턴.
CLAUDE.md 절대원칙 4("메인에 없는 터미널도 계속 실행") 충족.

**9) suppressed 패인이 사이드바에 보이는가 → YES**
`zellij-server-0.44.3/src/tab/mod.rs:5417` `Tab::pane_infos()`
```rust
for (_pane_id_of_suppressing_pane, (_is_scrollback_editor, pane)) in self.suppressed_panes.iter() {
    let mut pane_info_for_suppressed_pane = pane_info_for_pane(&pane.pid(), pane, &current_pane_group);
    pane_info_for_suppressed_pane.is_floating = false;
    pane_info_for_suppressed_pane.is_suppressed = true;
    pane_info_for_suppressed_pane.is_focused = false;
    pane_info_for_suppressed_pane.is_fullscreen = false;
    pane_info.push(pane_info_for_suppressed_pane);
}
```
→ `Event::PaneUpdate(PaneManifest)` 로 사이드바 플러그인에 **숨은 패인까지 전부** 전달된다.
`PaneInfo.is_suppressed` (`zellij-utils-0.44.3/src/data.rs:2307-2309`) 로 구분 가능.
`PaneManifest` 생성 지점: `zellij-server-0.44.3/src/screen.rs:3244`.

### 근거 B — 존재하는 패인 제어 API 목록 (`zellij-tile-0.44.3/src/shim.rs`)

`grep -n '^pub fn' src/shim.rs` 결과에서 패인 제어에 해당하는 것만 추림. 전부 **`ChangeApplicationState`**.

| 분류 | 시그니처 | 위치 |
|---|---|---|
| **교체** | `replace_pane_with_existing_pane(pane_id_to_replace: PaneId, existing_pane_id: PaneId, suppress_replaced_pane: bool)` | shim.rs:2711 |
| 포커스 | `focus_pane_with_id(pane_id: PaneId, should_float_if_hidden: bool, should_be_in_place_if_hidden: bool)` | shim.rs:1772 |
| 포커스 | `focus_terminal_pane(terminal_pane_id: u32, should_float_if_hidden: bool, should_be_in_place_if_hidden: bool)` | shim.rs:1387 |
| 포커스 | `focus_plugin_pane(plugin_pane_id: u32, should_float_if_hidden: bool, should_be_in_place_if_hidden: bool)` | shim.rs:1403 |
| 포커스 | `focus_next_pane()` / `focus_previous_pane()` / `move_focus(Direction)` / `move_focus_or_tab(Direction)` | 1096 / 1104 / 1112 / 1120 |
| 숨김/표시 | `hide_pane_with_id(pane_id: PaneId)` | shim.rs:886 |
| 숨김/표시 | `show_pane_with_id(pane_id: PaneId, should_float_if_hidden: bool, should_focus_pane: bool)` | shim.rs:902 |
| 숨김/표시 | `hide_self()` / `show_self(should_float_if_hidden: bool)` | 878 / 894 |
| 이동 | `move_pane_with_pane_id(pane_id: PaneId)` | shim.rs:2226 |
| 이동 | `move_pane_with_pane_id_in_direction(pane_id: PaneId, direction: Direction)` | shim.rs:2234 |
| 리사이즈 | `resize_pane_with_id(resize_strategy: ResizeStrategy, pane_id: PaneId)` | shim.rs:1764 |
| 리사이즈 | `resize_focused_pane(Resize)` / `resize_focused_pane_with_direction(Resize, Direction)` | 1075 / 1083 |
| 스택 | `stack_panes(pane_ids: Vec<PaneId>)` | shim.rs:2471 |
| 플로팅↔타일 | `toggle_pane_embed_or_eject_for_pane_id(pane_id: PaneId)` | shim.rs:2306 |
| 플로팅↔타일 | `float_multiple_panes(pane_ids: Vec<PaneId>)` / `embed_multiple_panes(pane_ids: Vec<PaneId>)` | 2591 / 2598 |
| 플로팅 | `change_floating_panes_coordinates(Vec<(PaneId, FloatingPaneCoordinates)>)` | shim.rs:2478 |
| 플로팅 | `set_floating_pane_pinned(pane_id: PaneId, should_be_pinned: bool)` | shim.rs:2464 |
| 플로팅 | `show_floating_panes(tab_id: Option<usize>) -> Result<bool, String>` / `hide_floating_panes(...)` | 2828 / 2851 |
| 풀스크린 | `toggle_pane_id_fullscreen(pane_id: PaneId)` | shim.rs:2298 |
| **swap layout** | `next_swap_layout()` / `previous_swap_layout()` — **인자 없음** | 1329 / 1321 |
| 레이아웃 강제 | `override_layout(layout_info, retain_existing_terminal_panes: bool, retain_existing_plugin_panes: bool, apply_only_to_active_tab: bool, context: BTreeMap<String,String>)` | shim.rs:2753 |
| 닫기 | `close_pane_with_id(pane_id: PaneId)` / `close_multiple_panes(Vec<PaneId>)` | 1753 / 2584 |
| 외형 | `rename_pane_with_id` / `set_pane_borderless(PaneId, bool)` / `set_pane_color(PaneId, Option<String>, Option<String>)` / `toggle_pane_borderless(PaneId)` | 2334 / 2503 / 2516 / 2491 |
| 그룹/하이라이트 | `group_and_ungroup_panes(Vec<PaneId>, Vec<PaneId>, for_all_clients: bool)` / `highlight_and_unhighlight_panes(Vec<PaneId>, Vec<PaneId>)` | 2558 / 2573 |
| 조회 | `get_pane_info(pane_id: PaneId) -> Option<PaneInfo>` | shim.rs:262 |
| 조회 | `get_focused_pane_info() -> Result<(usize, PaneId), String>` | shim.rs:207 |

`PaneId` = `enum { Terminal(u32), Plugin(u32) }` (`zellij-utils-0.44.3/src/data.rs:2827`)

### 근거 C — 존재하지 **않는** API (확인함)

| 없는 것 | 확인 방법 |
|---|---|
| swap layout을 **인덱스/이름으로 직접 지정**하는 함수 | `grep -in "swap_layout" src/shim.rs` → `previous_swap_layout`, `next_swap_layout` **2개뿐**. 순환 이동만 가능 |
| 타일 패인의 **절대 좌표/크기 직접 지정** (`set_pane_geom` / `move_pane_to` / `set_pane_position` / `set_pane_size`) | `grep -in "fn set_pane_geom\|fn move_pane_to\|set_pane_position\|fn set_pane_size" src/shim.rs` → **0건**. 타일 패인 배치는 레이아웃과 `resize_pane_with_id`(상대적 증감)로만 조작 |
| 좌표 지정은 **플로팅 패인만** 가능 | `change_floating_panes_coordinates` 만 존재 (shim.rs:2478) |
| 특정 패인을 **레이아웃의 N번 슬롯에 넣어라**는 명령형 API | 위 두 항목의 귀결. 슬롯 배정은 레이아웃 매칭 규칙(근거 D)에 위임됨 |

### 근거 D — permission 매핑

`zellij-server-0.44.3/src/plugins/zellij_exports.rs:5284` `fn check_command_permission`

`PermissionType::ChangeApplicationState` 로 매핑되는 커맨드(발췌, :5329-5432):
```
Resize / ResizeWithDirection / ResizePaneIdWithDirection
FocusNextPane / MoveFocus / MoveFocusOrTab
FocusTerminalPane / FocusPluginPane
MovePane / MovePaneWithDirection / MovePaneWithPaneId / MovePaneWithPaneIdInDirection
ShowPaneWithId / HidePaneWithId
ToggleFocusFullscreen / TogglePaneIdFullscreen
TogglePaneEmbedOrEject / TogglePaneEmbedOrEjectForPaneId
PreviousSwapLayout / NextSwapLayout
StackPanes / ChangeFloatingPanesCoordinates / SetFloatingPanePinned
FloatMultiplePanes / EmbedMultiplePanes / CloseMultiplePanes
ReplacePaneWithExistingPane          ← ★
OverrideLayout
ShowFloatingPanes / HideFloatingPanes
TogglePaneBorderless / SetPaneBorderless / SetPaneColor
GroupAndUngroupPanes / HighlightAndUnhighlightPanes
CloseTerminalPane / ClosePluginPane / CloseFocus
RenameTerminalPane / RenamePluginPane
SwitchTabTo / GoToTab / NewTab / ...
→ PermissionType::ChangeApplicationState
```

`PermissionType::ReadApplicationState`:
```
GetPaneInfo / GetTabInfo / GetFocusedPaneInfo / GetPanePid / GetPaneRunningCommand / GetPaneCwd
DumpLayout / DumpSessionLayout / ParseLayout / GetLayoutDir / ListClients / GetSessionList
→ PermissionType::ReadApplicationState
```

기타 polycanv가 쓸 것: `OpenCommandPane*`·`RunCommand`·`ExecCmd` → `RunCommands`,
`OpenTerminal*`·`StartOrReloadPlugin` → `OpenTerminalsOrPlugins`,
`Write`·`WriteChars`·`WriteToPaneId`·`WriteCharsToPaneId` → `WriteToStdin`.

Rust enum variant 이름: `zellij-utils-0.44.3/src/data.rs:1064-1065`
(`PermissionType::ReadApplicationState`, `PermissionType::ChangeApplicationState`)
proto 정의: `zellij-utils-0.44.3/src/plugin_api/plugin_permission.proto:5-23` (총 17종)

> 참고: `check_command_permission` 최상단(:5289)에 `if plugin_env.plugin.is_builtin() { return Granted }`
> 가 있다. polycanv 플러그인은 builtin이 아니므로 **반드시 `request_permission`** (shim.rs:91)을 호출해야 한다.

### 근거 E — swap layout이 패인을 죽이지 않으며 슬롯을 제어할 수 있는 근거

**호출 체인**
```
zellij-tile shim.rs:1329  next_swap_layout()
zellij-server zellij_exports.rs:3123  fn next_swap_layout(env)
zellij-server screen.rs:9857  tab.next_swap_layout()
zellij-server tab/mod.rs:1151  Tab::next_swap_layout()
zellij-server tab/mod.rs:1093  Tab::relayout_tiled_panes(search_backwards)
zellij-server tab/layout_applier.rs:160  LayoutApplier::apply_tiled_panes_layout_to_existing_panes()
```

**`Tab::relayout_tiled_panes`** (`tab/mod.rs:1093-1141`) — spawn도 kill도 없다:
```rust
if let Some(layout_candidate) = self.swap_layouts.swap_tiled_panes(&self.tiled_panes, search_backwards) {
    let application_res = LayoutApplier::new(...).apply_tiled_panes_layout_to_existing_panes(&layout_candidate);
    if application_res.is_err() { self.swap_layouts.set_is_tiled_damaged(); ... }
} else {
    self.swap_layouts.set_is_tiled_damaged();
}
self.tiled_panes.reapply_pane_frames();
self.tiled_panes.resize(display_area);
```

**슬롯 배정 규칙** (`tab/layout_applier.rs:160-234`) — 주석까지 그대로:
```rust
let mut existing_tab_state = ExistingTabState::new(self.tiled_panes.drain());  // ← 기존 패인 수거

// look for exact matches (eg. panes that expect a specific command or plugin to run in them)
existing_tab_state.find_and_extract_exact_match_pane(&layout.run, position_and_size.logical_position)

// look for matches according to the logical position in the layout
existing_tab_state.find_and_extract_pane_with_same_logical_position(position_and_size.logical_position)

// fill the remaining panes by order of their logical position
existing_tab_state.find_and_extract_pane(position_and_size.logical_position)

// add the rest of the panes where tiled_panes finds room for them (eg. if the layout had
// less panes than we've got in our state)
pane_applier.handle_remaining_tiled_pane_ids(remaining_pane_ids, existing_tab_state, None, None);
```

**1단계 exact match의 실제 비교 로직** (`tab/layout_applier.rs:1218` `find_pane_id_with_same_contents`):
```rust
if run.is_none() { return None; }
let panes_with_same_contents = candidates.iter()
    .filter(|(_pid, p)| p.invoked_with() == run)          // ← Run 동등 비교
    .collect::<Vec<_>>();
if panes_with_same_contents.len() > 1 {
    // 같은 command가 여럿이면 logical_position 으로 tie-break
    ...find(|(_pid, p)| p.position_and_size().logical_position == pane_logical_position)
}
```
`Run` = `enum { Plugin(RunPluginOrAlias), Command(RunCommand), EditFile(..), Cwd(PathBuf) }`
(`zellij-utils-0.44.3/src/input/layout.rs:256`)

→ **KDL 레이아웃에서 `command "claude"` 또는 `plugin location="file:.../sidebar.wasm"` 를 특정 슬롯에
써두면, 전환 시 그 패인이 그 슬롯으로 간다.** 같은 command가 여러 개면 `logical_position` 이 tie-break.

**중요한 한계**: 수거 대상이 `self.tiled_panes.drain()` 뿐이다. `suppressed_panes` 도 `floating_panes` 도
포함되지 않는다. → **swap layout은 숨은 패인을 끌어올리지 못한다.**

### 근거 F — 런타임 스파이크 (2026-08-19)

> 여기부터가 **[런타임]** 근거다. 위의 A~E는 전부 **[정적]** 이다.

#### F-0. 테스트 장치

폐기용 플러그인 `spike` 를 스크래치패드에 만들어 실제로 빌드했다(프로젝트 트리는 건드리지 않음).
플러그인이 하는 일은 **딱 두 가지** — 파이프로 받은 명령대로 API를 호출하고, `PaneManifest` 를 로그로 덤프.
**관측은 대부분 플러그인 바깥의 zellij CLI로 했다** (자기가 자기를 검증하는 순환을 피하려고):

| 관측 대상 | 수단 | 성격 |
|---|---|---|
| 프로세스 생존 | `ps -eo pid,command \| grep SPIKEMARKER` | 외부 (OS) |
| 스크롤백 보존 | `zellij action dump-screen -p terminal_N -f` | 외부 (zellij CLI) |
| 패인 목록 | `zellij action list-panes` | 외부 (zellij CLI) |
| 지오메트리 · `is_suppressed` | 플러그인의 `PaneUpdate` → `eprintln!` → zellij 로그 | 내부 (유일하게 내부 수단) |

```
$ cargo build --release --target wasm32-wasip1
   Compiling zellij-utils v0.44.3
   Compiling zellij-tile v0.44.3
   Compiling spike v0.1.0
    Finished `release` profile [optimized] target(s) in 49.58s
```

테스트 레이아웃 (사이드바 1 + 터미널 2, polycanv 구조의 최소 모형):
```kdl
layout {
    pane size=3 borderless=true { plugin location="file:.../spike.wasm" }
    pane split_direction="vertical" {
        pane name="MAINSLOT" command="bash" {
            args "-c" "for i in $(seq 1 60); do echo ALPHA_LINE_$i; done; echo SPIKEMARKER_ALPHA_0001; sleep 9999"
        }
        pane name="SIDESLOT" command="bash" {
            args "-c" "for i in $(seq 1 60); do echo BETA_LINE_$i; done; echo SPIKEMARKER_BETA_0002; sleep 9999"
        }
    }
}
```
각 터미널이 60줄을 출력한 뒤 `sleep 9999` 로 살아있는다. 60줄은 패인 높이(47행)보다 많아서
**일부가 실제 스크롤백으로 밀려난다** — `dump-screen -f`(full scrollback) 검증이 의미를 갖게 하려는 설계다.

전용 세션 `polycanv-spike` 만 사용했고, 모든 명령에 `--session polycanv-spike` 를 붙였다.
(테스트 중 다른 에이전트들의 세션 `polycanv-layout-poc`, `polycanv-scout2-hooks` 등이 동시에 떠 있었으나
한 번도 건드리지 않았다.)

#### F-1. 왕복 2회 — `auto_layout false`

호출: `replace_pane_with_existing_pane(target, with, suppress=true)` 를 4회.
`t1`=MAINSLOT(터미널 1), `t2`=SIDESLOT(터미널 2).

| 라운드 | 호출 | 결과 (`is_suppressed`) |
|---|---|---|
| 시작 | — | t1 `false` / t2 `false` |
| R1 | `replace(t1, t2, true)` | t2 `false`(focused) / **t1 `true`** |
| R2 | `replace(t2, t1, true)` | **t1 `false`**(focused) / t2 `true` ← **suppressed 회수 경로** |
| R3 | `replace(t1, t2, true)` | t2 `false`(focused) / t1 `true` |
| R4 | `replace(t2, t1, true)` | t1 `false`(focused) / t2 `true` |

플러그인 로그(호출이 실제로 발생했음):
```
DEBUG |...| SPIKE_CALL: replace_pane_with_existing_pane(Terminal(1), Terminal(2), true)
DEBUG |...| SPIKE_CALL: returned
```

R4 시점의 `PaneManifest` 덤프 (지오메트리 + suppressed):
```
SPIKE_DUMP[NOAUTO_R4]: tab=1 id=terminal_1 title="MAINSLOT" suppressed=false focused=true  floating=false x=0 y=3 rows=47 cols=200
SPIKE_DUMP[NOAUTO_R4]: tab=1 id=terminal_2 title="SIDESLOT" suppressed=true  focused=false floating=false x=0 y=3 rows=47 cols=200
SPIKE_DUMP[NOAUTO_R4]: tab=1 id=plugin_3   title="file:.../spike.wasm"       suppressed=false focused=false floating=false x=0 y=0 rows=3 cols=200
```

**(a) 프로세스 생존 — PASS.** 시작 시 PID와 4회 교체 후 PID가 동일:
```
$ ps -eo pid,command | grep SPIKEMARKER | grep -v grep | awk '{print $1}'
# 시작:        36625 / 36626
# 왕복 2회 후: 36625 / 36626
```

**(b) 스크롤백 보존 — PASS.** suppressed 상태의 패인도 `dump-screen -f` 로 전체 스크롤백이 나온다:
```
--- scrollback terminal_1 (ALPHA) ---
  total ALPHA_LINE_ matches: 60
  first/last/marker: ALPHA_LINE_1 ALPHA_LINE_60 SPIKEMARKER_ALPHA_0001
--- scrollback terminal_2 (BETA) ---
  total BETA_LINE_ matches: 60
  first/last/marker: BETA_LINE_1 BETA_LINE_60 SPIKEMARKER_BETA_0002
```
60줄 전부 + 마커가 4회 교체 내내 양쪽 다 보존됐다. 화면 밖으로 밀려난 줄까지 남아 있다.

**(c) 지오메트리 — 조건부 PASS.**
- **사이드바(플러그인) 패인은 완전히 유지**: `x=0 y=0 rows=3 cols=200` 이 4회 내내 불변. ← polycanv에 중요한 건 이쪽
- **메인 콘텐츠 패인**: `x=0 y=3 rows=47` 유지, 단 `cols` 가 `100 → 200` 으로 **넓어진다**.
  이건 버그가 아니라 당연한 결과다 — 짝이던 tiled 패인이 tiled 집합에서 빠지면서 남은 패인이 그 공간을
  흡수한다. polycanv의 리스트 뷰는 어차피 "사이드바 + 콘텐츠 패인 1개"이므로 **의도한 동작과 일치**한다.
  "A의 geom을 B가 승계한다"(정적 근거 A-7)와 "남은 tiled 공간이 재분배된다"는 둘 다 참이며 순서대로 일어난다.

**부수 확인 — suppressed 패인은 `PaneManifest` 에 나온다.** 정적 근거 A-9의 런타임 확인이다.
심지어 내가 만들지 않은 zellij 빌트인 패인에서도 관측됐다:
```
SPIKE_DUMP[BASELINE]: tab=0 id=plugin_0 title="(.) - zellij:link" suppressed=true focused=false ...
```

#### F-2. 같은 시퀀스 — `auto_layout true`

동일한 4회 교체를 `auto_layout true` 로 반복했다. (a)(b)는 동일하게 PASS지만 **(c)가 깨진다**:

```
# auto_layout=true, BASELINE
SPIKE_DUMP[BASELINE]: tab=1 id=terminal_1 MAINSLOT suppressed=false x=0 y=3 rows=47 cols=100
SPIKE_DUMP[BASELINE]: tab=1 id=terminal_2 SIDESLOT suppressed=false x=100 y=3 rows=47 cols=100
SPIKE_DUMP[BASELINE]: tab=1 id=plugin_3   spike.wasm              x=0 y=0 rows=3  cols=200

# auto_layout=true, ROUND1 이후
SPIKE_DUMP[ROUND1]: tab=1 id=terminal_2 SIDESLOT suppressed=false x=0 y=1 rows=49 cols=200
SPIKE_DUMP[ROUND1]: tab=1 id=terminal_1 MAINSLOT suppressed=true  x=0 y=1 rows=49 cols=200
SPIKE_DUMP[ROUND1]: tab=1 id=plugin_3   spike.wasm               x=0 y=0 rows=1  cols=200   ← 3행에서 1행으로 찌그러짐
```
사이드바가 `rows=3` → `rows=1` 로 붕괴하고 탭 전체가 swap layout으로 재배치됐다.
정적 근거에서 예고했던 `Tab::extract_pane` 의 `auto_layout` 재배치 트리거(`tab/mod.rs:4114-4119`)가
실제로 발동한 것이다. → **결론 3: `auto_layout false` 강제.**

#### F-3. upstream이 `// TODO: test this` 로 남긴 경로 — 이제 검증됨

1차 문서의 미해결 2번이었다. **별도 테스트가 필요 없다 — 왕복의 복귀 단계(R2/R4)가 정확히 그 경로다.**

논증: R1에서 `t1` 이 `suppressed=true` 가 된다. R2는 `replace_pane_with_existing_pane(t2, t1, true)` 로
`t1` 을 **꺼내온다**. `Screen::replace_pane_with_existing_pane` 은 `extract_pane(t1, dont_swap_if_suppressed=true)`
를 호출하는데(screen.rs:4443), `t1` 은 floating도 tiled도 아니고 suppressed이므로 **`tab/mod.rs:4125-4133`
의 세 번째 분기 외에는 도달할 경로가 없다.** R2가 성공했다는 것은 그 분기가 동작했다는 것과 동치다.
R2와 R4에서 각각 성공했고, `auto_layout` 두 설정 모두에서 성공했다 — 총 4회.

#### F-4. `override_layout` — `retain_*` 플래그의 실제 시맨틱 (1차 미해결 8번)

먼저 **CLI로는 테스트할 수 없다**는 걸 확인했다. `zellij action override-layout` 은 임시 클라이언트로
접속하기 때문에 활성 탭이 없다:
```
ERROR |zellij_server::screen| zellij-server/src/screen.rs:7635:
      Failed to override layout of active tab: active tab not found for client 2
```
→ 그래서 스파이크 플러그인 안에서 직접 호출했다. 적용 레이아웃은 `layout { pane }` (패인 1개).

**케이스 1 — `retain_terminals=false, retain_plugins=false` (CLI 기본값): 패인이 제거된다**
```
$ # 호출 전
PANE_ID   TYPE     TITLE
plugin_3  plugin   file:.../spike.wasm
terminal_1 terminal MAINSLOT
terminal_2 terminal SIDESLOT

DEBUG |...| SPIKE_CALL: override_layout(Stringified, retain_terminals=false, retain_plugins=false, active_tab_only=true)
INFO  |zellij_server::plugins::w| wasm_bridge.rs:494: Bye from plugin 3     ← 호출한 플러그인 자신이 죽음

$ # 호출 후
PANE_ID   TYPE     TITLE
terminal_0 terminal Pane #1
terminal_3 terminal Pane #1      ← 새로 생성된 패인만 남음
```
`plugin_3`·`terminal_1`·`terminal_2` 전부 탭에서 사라졌다. **호출한 플러그인 자신의 패인까지 닫힌다.**
(OS 프로세스는 직후까지 살아 있었으나 세션에서 도달 불가 상태가 됐다 — 아래 미해결 참조.)

**케이스 2 — `retain_terminals=true, retain_plugins=true`: 전부 보존된다**
```
DEBUG |...| SPIKE_CALL: override_layout(Stringified, retain_terminals=true, retain_plugins=true, active_tab_only=true)

$ # 호출 후
plugin_3   plugin   file:.../spike.wasm      ← 살아있음
terminal_1 terminal MAINSLOT                 ← 살아있음
terminal_2 terminal SIDESLOT                 ← 살아있음
terminal_3 terminal Pane #1                  ← 레이아웃 노드만큼 새로 추가됨
$ ps ... # PID 42259 / 42260 그대로
$ grep -c "Bye from plugin 3" zellij.log
0
```
→ **`retain_*` 를 둘 다 `true`** 로 주면 절대원칙 2를 지킨다. 단 레이아웃의 패인 노드 수만큼
**새 패인이 추가로 생긴다**는 점에 유의.

#### F-5. 스파이크 중 발견한 실무 함정 4개 (구현 시 그대로 부딪힌다)

1. **플러그인은 `cdylib` 이 아니라 바이너리 크레이트여야 한다.**
   `crate-type = ["cdylib"]` 로 빌드하면 zellij가 로드에 실패한다:
   `failed to load plugin from instance ... Caused by: could not find exported function`.
   `plugin_loader.rs:176` 이 **`_start`** 를 요구하는데 cdylib에는 없다.
   → `src/main.rs` + `register_plugin!` (매크로가 `main` 을 정의하므로 직접 쓰면 `E0428` 중복 오류).
2. **wasm 타깃은 `wasm32-wasip1`.** 구 이름 `wasm32-wasi` 는 이 툴체인에 없다.
   (CLAUDE.md는 이미 갱신돼 있다 — 다만 「검증」 항의 예시 한 줄에 `wasm32-wasi` 가 남아 있어
   문서 내부에서 상충한다. 사소하지만 정리하면 좋다.)
3. **권한 캐시 키는 `file:` 접두어 없는 맨 경로다.**
   `~/Library/Caches/org.Zellij-Contributors.Zellij/permissions.kdl` 의 노드 이름은
   `RunPluginLocation::File(path)` 의 Display(`layout.rs:656-663`) = **맨 경로**.
   `"file:/path/to.wasm"` 로 적으면 매칭되지 않아 권한이 조용히 안 붙는다. `"/path/to.wasm"` 이 맞다.
4. **`load()` 에서 `subscribe()` 를 `request_permission()` 보다 먼저 호출해야 한다.**
   캐시에 권한이 이미 있으면 서버가 `PermissionRequestResult(Granted)` 를 **즉시** 보낸다
   (`zellij_exports.rs:894-907`). 구독이 늦으면 그 이벤트를 놓친다.

---

## 설계에 미치는 영향

### 영향 1 — 리스크 1은 **해소**. 우회 설계를 세우지 않아도 된다

CLAUDE.md 「미검증 리스크」의 "[높음] 리스트 선택 → 메인 패인 교체 ... 불가 시 swap layout + 포커스 이동
조합으로 우회" 는 **우회가 불필요**한 것으로 정리된다. 전용 API가 있고, 그것이 정확히 그 시맨틱이며,
**런타임 왕복 2회로 검증됐다**(근거 F-1). CLAUDE.md의 해당 리스크 항목은 **닫아도 된다.**

### 영향 1-b — config에 `auto_layout false` 를 못박아야 한다 **[런타임]**

이번 스파이크에서 새로 나온 필수 제약이다. `auto_layout true` 면 교체 한 번에 사이드바 패인이
`rows=3` → `rows=1` 로 붕괴한다(근거 F-2). polycanv가 배포하는 기본 config에 반드시 포함할 것:

```kdl
auto_layout false
```

**이미 반영돼 있다** — `config/keybinds.kdl:27` 에 `auto_layout false` 가 들어가 있는 것을 확인했다.
이 문서는 그 설정이 **왜 필수인지에 대한 런타임 근거**를 제공한다. 되돌리지 마라.
(`config/` 는 내 소유 경계 밖이라 읽기만 했다.)

### 영향 2 — sidebar 플러그인의 선택 핸들러는 사실상 한 줄

```rust
// plugins/sidebar/ — 사용자가 사이드바에서 항목 선택 시
use zellij_tile::prelude::*;

// self.main_slot_pane_id: 현재 메인 슬롯을 점유 중인 패인
// selected: 사용자가 고른 (숨어있을 수도 있는) 패인
replace_pane_with_existing_pane(
    self.main_slot_pane_id,   // 밀려날 패인
    selected,                 // 올라올 패인
    true,                     // ★ 반드시 true — false 면 밀려난 패인이 닫힌다
);
self.main_slot_pane_id = selected;   // 다음 교체를 위해 슬롯 점유자 갱신
```
- 포커스 이동은 `TiledPanes::replace_pane` 내부의 `move_clients_between_panes` 가 처리하므로
  **`focus_pane_with_id` 추가 호출 불필요**. **[런타임 확인]** — 교체 후 매번 새 패인이
  `focused=true` 로 나왔다(근거 F-1의 덤프).
- 메인 슬롯의 좌표/크기 계산 불필요 — `set_geom(removed_pane_geom)` 로 자동 승계. **[런타임 확인]**
  단 `auto_layout false` 가 전제다.

**핵심 상태 1개만 관리하면 된다: "지금 메인 슬롯에 있는 PaneId".**
이건 `PaneUpdate` 에서 `is_suppressed == false && !is_plugin` 인 패인으로도 복원 가능하므로,
plugin 재시작 시에도 재구성할 수 있다.

→ **protocol 변경 요청 후보**: `crates/protocol/` 의 패인 메타데이터에
`main_slot_pane_id: Option<PaneId>` 성격의 필드가 필요할 수 있다. (리드 판단 사항. 나는 수정하지 않았다.)

### 영향 3 — 신호등(status) 플러그인은 숨은 터미널도 그대로 감시할 수 있다

`Tab::pane_infos()` 가 suppressed 패인을 `is_suppressed: true` 로 포함해 `PaneManifest` 에 실어 보낸다.
따라서 status 플러그인은 **메인에 없는 터미널의 상태 변화도 이벤트로 받는다** — CLAUDE.md 절대원칙 4를
추가 장치 없이 만족한다.

`PaneInfo` 에서 상태 판별에 쓸 만한 필드 (`zellij-utils-0.44.3/src/data.rs:2296-2347`):
`id`, `is_plugin`, `is_focused`, `is_suppressed`, `title`, `exited`, `exit_status: Option<i32>`,
`is_held`, `terminal_command: Option<String>`, `plugin_url`, `is_selectable`.

⚠️ 단 `PaneInfo` 에는 **출력 내용이나 마지막 활동 시각이 없다.** 상태 판별 우선순위 ②(출력 패턴 정규식)·
③(벨)·④(idle 휴리스틱)는 `PaneUpdate` 만으로는 구현 불가다. 별도 수단이 필요하다
(`get_pane_scrollback` shim.rs:1812 / `set_pane_regex_highlights` shim.rs:2884 등). status-detector 조사 몫.

### 영향 4 — 레이아웃(KDL) 작성 규칙

layout-view 담당이 지켜야 할 것:

1. **메인 슬롯 노드에는 `command`/`plugin` 을 쓰지 마라.** exact match(1단계)에 걸리면 swap layout 전환 때
   특정 패인이 강제로 그 슬롯에 배정되어, `replace_pane_with_existing_pane` 로 만들어둔 "지금 메인에 뭐가
   있는지" 상태를 레이아웃이 덮어쓴다.
2. **사이드바 슬롯에는 `plugin location=...` 을 명시하라.** 그래야 캔버스↔리스트 전환 시 사이드바 플러그인
   패인이 항상 사이드바 자리로 되돌아간다.
3. `next_swap_layout()` / `previous_swap_layout()` 은 **인덱스 지정이 불가능한 순환 전환**이다.
   캔버스·리스트 **정확히 2개**의 swap layout만 두면 `next_swap_layout()` 이 토글로 동작한다.
   3개 이상 두면 "리스트로 가라"를 결정론적으로 표현할 수 없다.
4. swap layout은 **suppressed 패인을 건드리지 않는다.** 캔버스 뷰에서 여러 패인을 동시에 보여주려면,
   숨겨둔 패인들을 먼저 `replace_pane_with_existing_pane` 등으로 tiled 로 되돌린 뒤 전환해야 한다.
   **추정: "캔버스 = 전부 tiled / 리스트 = 1개만 tiled + 나머지 suppressed" 라는 두 모드 사이의 전환은
   swap layout 하나로는 부족하고, 플러그인이 패인 집합을 먼저 맞춘 뒤 swap layout을 호출하는
   2단계가 된다.** (미검증 — 「미해결」 참조)

### 영향 5 — 최소 지원 버전

| API | 최초 등장 |
|---|---|
| `replace_pane_with_existing_pane` | **0.43.0** |
| `stack_panes` | 0.42.x |
| `override_layout` | **0.44.0** |

확인 명령:
```
$ grep -c 'pub fn replace_pane_with_existing_pane' zellij-tile-<v>/src/shim.rs
0.40.1: 0    0.41.2: 0    0.42.2: 0    0.43.0: 1    0.43.1: 1    0.44.0: 1    0.44.3: 1
$ grep -c 'fn override_layout' ...
0.43.1: 0    0.44.0: 1
```
→ **`zellij >= 0.43.0` 을 최소 요구사항으로 명시**해야 한다. `override_layout` 까지 쓸 거면 `>= 0.44.0`.
현 시점 최신인 **0.44.3 고정을 권장**한다(Windows 네이티브 지원 검증도 최신 기준이 유리).

### 영향 6 — 대안 B (백업안, 직접 API 실패 시)

`replace_pane_with_existing_pane` 이 실패하면 swap layout이 아니라 아래 조합을 쓴다.
**swap layout 단독 우회는 성립하지 않는다** (근거 E의 한계).

```rust
// B가 메인으로 올라오게 하고 A를 살려둔 채 숨긴다
show_pane_with_id(selected_b, /* should_float_if_hidden */ false, /* should_focus_pane */ true);
hide_pane_with_id(current_a);
```
- `show_pane_with_id` (shim.rs:902) 는 suppressed 를 해제하고 표시, `should_focus_pane: true` 로 포커스까지.
- `hide_pane_with_id` (shim.rs:886) 로 A를 다시 숨긴다.
- **차이점**: 이 조합은 A의 **지오메트리를 B가 승계하지 않는다.** B가 tiled 어딘가에 새로 자리를 잡고,
  A가 빠지면서 레이아웃이 재계산된다 → "메인 슬롯 크기 유지"가 깨질 수 있다. 그래서 **1순위는 아니다.**
- 순서 주의: `hide` 를 먼저 하면 tiled 패인이 0개가 되는 순간이 생길 수 있으므로 **`show` → `hide` 순서.**

**추정**: 대안 B 사용 시 `hide_pane_with_id` 직후 `next_swap_layout()` 을 한 번 호출해 레이아웃을 재적용하면
슬롯 형태를 복원할 수 있다. (미검증)

---

## 해결된 항목 (1차 문서의 미해결 → 근거로 승격)

| 1차 미해결 | 상태 | 근거 |
|---|---|---|
| 1. 런타임 검증이 전무하다 | **해결** — 왕복 2회 실행 검증 완료 | 근거 F-1 |
| 2. upstream `// TODO: test this` 경로 | **해결** — 4회 성공. 별도 테스트 불필요(왕복 복귀 단계가 그 경로) | 근거 F-3 |
| 4. `auto_layout` 부작용 | **해결** — 부작용 실재. `auto_layout false` 강제 결론 | 근거 F-2, 결론 3 |
| 8. `override_layout` 이 패인을 닫는지 | **해결** — `retain_*=false` 면 닫힌다(호출자 자신까지). `true` 면 보존 | 근거 F-4 |

---

## 미해결

1. **`override_layout(retain=false)` 이후 남은 OS 프로세스의 최종 운명을 확인하지 않았다.**
   근거 F-4 케이스 1에서 패인은 탭에서 사라졌지만 `ps` 상 bash 프로세스 2개는 **직후까지 살아 있었다**
   (ppid는 여전히 zellij 서버). 이게 (a) 지연 정리인지 (b) 좀비/고아로 영구히 남는 누수인지
   확인하지 않았다. **polycanv는 `retain=true` 만 쓸 것이므로 실사용 영향은 없지만**, 만약
   `override_layout` 을 쓰게 된다면 이 지점을 먼저 확인할 것.

2. **`Tab::get_pane_info` 의 suppressed 조회는 키 기준이라 경로에 따라 실패할 수 있다.**
   `tab/mod.rs:3921` 은 `self.suppressed_panes.get(&pane_id)` 로 **맵의 키**를 찾는다. 반면
   `pane_infos()` (:5417) 와 `extract_pane` (:4128) 은 **`pane.pid()` 값**으로 찾는다.
   `suppress_pane_and_replace_with_other_pane` 경로에서는 키 == 자기 pid 라 일치하지만
   (`insert_suppressed_pane(replaced_pane.pid(), ...)`, :2347), 스크롤백 에디터 경로에서는 키가
   "억누른 쪽" 패인 id다. → **단건 조회 `get_pane_info(PaneId)` 대신 `PaneUpdate`/`PaneManifest` 를
   신뢰하라.** 실제 영향 범위는 여전히 미검증(런타임에서 이 API를 쓰지 않았다).

3. **swap layout 자체를 런타임으로 검증하지 않았다.** 근거 E(슬롯 배정 규칙, 패인을 죽이지 않음)는
   **전부 정적 판독**이다. 이번 스파이크는 `replace_pane_with_existing_pane` 과 `override_layout` 만
   실행했다. 캔버스↔리스트 전환을 swap layout으로 구현한다면 **layout-view 담당이 별도 스파이크 필요**.
   특히 "`command`/`plugin` exact match로 슬롯을 고정한다"는 결론(근거 E)은 실행으로 확인되지 않았다.

4. **영향 4-(4) "캔버스↔리스트 2단계 전환"은 여전히 추정이다.** suppressed 패인 집합을 맞춘 뒤
   swap layout을 호출하는 순서가 실제로 필요한지, 아니면 레이아웃 노드 수 차이만으로 충분한지
   검증되지 않았다. 위 3번과 함께 스파이크할 것.

5. **여러 탭에 걸친 동작 미검증.** `Screen::replace_pane_with_existing_pane` 은 두 패인이 **서로 다른 탭**에
   있어도 동작하도록 작성돼 있다(탭을 각각 찾아 extract 후 insert, screen.rs:4416-4446). 이번 스파이크는
   **모두 같은 탭 안에서만** 했다. 다중 탭 구성으로 갈 경우 별도 검증 필요.

6. **Windows 네이티브 동작 미검증.** 이번 스파이크는 macOS(Darwin 24.6.0)에서만 돌렸다.
   CLAUDE.md의 별도 [중] 리스크로 남아 있다.

7. **장시간·다수 패인 부하는 확인하지 않았다.** 터미널 2개, 교체 4회, 수 분 규모의 테스트다.
   polycanv의 실사용(터미널 6~8개, 수 시간 세션)에서 suppressed 패인이 쌓였을 때의 메모리·렌더링
   거동은 미확인. **추정:** suppressed 패인도 터미널 버퍼를 계속 유지하므로 메모리는 보이는 패인과
   동등하게 쓸 것이다.

8. **`suppress_replaced_pane: false` 분기는 런타임으로 시험하지 않았다.** 파괴적이라 의도적으로 뺐다.
   "패인이 닫힌다"는 것은 정적 판독(`close_pane_and_replace_with_other_pane`) 근거뿐이다.

---

## 부록 — 재현 절차

```bash
S=/tmp/zellij-src && mkdir -p $S && cd $S
UA="polycanv-research (your-email@example.com)"   # crates.io 는 User-Agent 를 요구한다

# 최신 버전 확인
curl -sS -A "$UA" https://index.crates.io/ze/ll/zellij-tile | tail -1

# 소스 확보
for c in zellij-tile zellij-utils zellij-server; do
  curl -sS -A "$UA" -L https://static.crates.io/crates/$c/$c-0.44.3.crate -o $c.crate
  tar xzf $c.crate
done

# 핵심 지점
grep -n '^pub fn' zellij-tile-0.44.3/src/shim.rs                       # API 전체 목록
sed -n '2711,2725p' zellij-tile-0.44.3/src/shim.rs                     # replace_pane_with_existing_pane
sed -n '5284,5450p' zellij-server-0.44.3/src/plugins/zellij_exports.rs # permission 매핑
sed -n '4408,4467p' zellij-server-0.44.3/src/screen.rs                 # Screen 측 구현
sed -n '4061,4137p' zellij-server-0.44.3/src/tab/mod.rs                # extract_pane (suppressed 회수)
sed -n '2330,2350p' zellij-server-0.44.3/src/tab/mod.rs                # suppress_pane_and_replace_with_other_pane
sed -n '160,234p'  zellij-server-0.44.3/src/tab/layout_applier.rs      # swap layout 슬롯 배정
sed -n '5417,5436p' zellij-server-0.44.3/src/tab/mod.rs                # pane_infos (suppressed 노출)
```

GitHub 대응 경로: `https://github.com/zellij-org/zellij/blob/v0.44.3/<crate>/src/...`

### 런타임 스파이크 재현 (근거 F)

스파이크 산출물은 전부 **스크래치패드**에 있고 프로젝트 트리에는 아무것도 남기지 않았다.
세션·프로세스·권한 파일은 모두 정리했다. 재현하려면:

**1) 플러그인 (바이너리 크레이트여야 한다)**
```toml
# Cargo.toml — [lib] crate-type = ["cdylib"] 을 쓰면 안 된다 (_start 없음 → 로드 실패)
[package]
name = "spike"
version = "0.1.0"
edition = "2021"
[dependencies]
zellij-tile = "0.44.3"
```
`src/main.rs` 에 `register_plugin!(State);` — 매크로가 `main` 을 정의하므로 직접 쓰지 말 것.
`load()` 에서는 **`subscribe()` 를 `request_permission()` 보다 먼저** 호출한다.

```bash
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
cargo build --release --target wasm32-wasip1     # wasm32-wasi 아님
```

**2) 권한 사전 부여** — 노드 이름은 `file:` 접두어 **없는 맨 경로**
```bash
cat > ~/Library/Caches/org.Zellij-Contributors.Zellij/permissions.kdl <<EOF
"$PWD/target/wasm32-wasip1/release/spike.wasm" {
    ReadApplicationState
    ChangeApplicationState
}
EOF
```

**3) 헤드리스 실행** — zellij 클라이언트는 tty가 필요하다. `pty.openpty()` 로 크기를 고정한 뒤
`TIOCSCTTY` 로 제어 터미널을 붙이고, 부모는 마스터를 계속 비워주면서 zellij가 보내는
터미널 질의(`\x1b[?996n`, `\x1b[6n`, `\x1b[?N$p` 등)에 답해준다. 답하지 않으면 클라이언트가
`Client sent over 1000 consecutive unknown messages` 로 쫓겨난다.

```bash
# 주의: --session 과 -n/--new-session-with-layout 조합은 레이아웃이 적용되지 않았다.
#       빈 세션을 먼저 띄우고 new-tab 으로 레이아웃을 넣는 쪽이 확실하다.
python3 launch2.py tty.log /opt/homebrew/bin/zellij \
    --config spike-config-noauto.kdl --session polycanv-spike &
zellij --session polycanv-spike action new-tab --layout spike-layout.kdl
```

**4) 구동과 관측**
```bash
# 파이프는 플러그인이 unblock 하지 않으면 블로킹된다 → 백그라운드로 던진다
( zellij --session polycanv-spike pipe --plugin "file:$WASM" \
      --name replace -- "terminal_1,terminal_2,true" & )

zellij --session polycanv-spike action list-panes
zellij --session polycanv-spike action dump-screen -p terminal_1 -f
ps -eo pid,command | grep SPIKEMARKER
grep SPIKE_DUMP "${TMPDIR}zellij-501/zellij-log/zellij.log"
```
플러그인 로그(`eprintln!`)는 **`$TMPDIR/zellij-<uid>/zellij-log/zellij.log`** 로 간다
(macOS에서는 `~/Library/Caches/...` 가 아니다).

**5) 정리**
```bash
zellij kill-session polycanv-spike && zellij delete-session polycanv-spike --force
ps -eo pid,command | grep SPIKEMARKER | grep -v grep | awk '{print $1}' | xargs -r kill
rm -f ~/Library/Caches/org.Zellij-Contributors.Zellij/permissions.kdl   # 원래 없던 파일
```
