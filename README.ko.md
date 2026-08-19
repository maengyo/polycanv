# polycanv

> ⚠️ 개발 중입니다. Windows / Linux 는 아직 검증되지 않았습니다.

**English: [README.md](README.md)**

작업하다 보면 세션이 여기저기 흩어집니다. 창 하나에 하나, 탭에 하나, 또 하나는 다른 창들 뒤에.
특정 세션을 찾으려면 결국 뒤지게 됩니다.

polycanv 는 그것들을 **한 화면**에 모읍니다. 펼쳐서 전부 한눈에 보거나, 리스트로 접고 하나에
집중하거나 — 어느 쪽이든 **각 세션이 어디 있는지 늘 알 수 있습니다.**

```
[캔버스 뷰]                        [리스트 뷰]
┌────────┬────────┐              ┌───────┬─────────────┐
│ claude │ codex  │   Ctrl+y    │ claude│             │
├────────┼────────┤ ───────────▶ │ codex │ claude code │
│ qwen   │ pwsh   │              │ qwen  │  (확대 표시) │
└────────┴────────┘              │ pwsh  │             │
                                 └───────┴─────────────┘
   전부 한눈에                      하나에 집중
```

- **캔버스 뷰** — 모든 세션이 한 화면에 타일로 펼쳐집니다. 아무거나 클릭하면 바로 입력됩니다.
- **리스트 뷰** — 사이드바에 세션별로 이름 · 도구 · 작업 디렉터리가 한 줄씩 보이고,
  오른쪽에 하나가 확대됩니다. 목록에서 고르면 그 세션이 메인 자리로 올라옵니다.
- **`Ctrl+y` 로 전환** — 보고 있던 세션이 메인 자리에 그대로 남습니다. 맥락이 끊기지 않습니다.
- **아무것도 닫히지 않습니다.** 리스트 뷰는 나머지를 *접는* 것이고, 전부 계속 돌아갑니다.

### 신호등

세션마다 불이 하나씩 붙습니다: 🟢 실행 중 / 🟡 입력 대기(권한 승인 등) / 🔴 완료 / ⚪ 유휴.

**지금 화면에 안 보이는 세션도** 사이드바에 불이 뜹니다 — 접어둔 사이에 끝난 세션을 놓치지
않습니다. 빨간불은 그 세션을 실제로 열어보면 꺼집니다.

## 안에서 무엇을 돌릴 수 있나

claude code · codex cli · opencode · qwen code · PowerShell · bash/zsh —
그리고 **설정에 한 줄 추가하면 무엇이든**. 위 목록은 기본값일 뿐 특별 대우 대상이 아닙니다.

## 설치

**요구사항**: [zellij](https://github.com/zellij-org/zellij) **0.43.0+** (0.44.3 권장),
Rust stable + `wasm32-wasip1` 타깃

```sh
git clone https://github.com/maengyo/polycanv && cd polycanv
sh scripts/install.sh
zellij --config config/keybinds.kdl -s polycanv -n layouts/polycanv.kdl
```

> **최초 실행 시 사이드바가 권한을 요청합니다. `y` 로 승인하세요.**
> 승인 전에는 로드만 되고 키에 반응하지 않습니다 — 고장이 아닙니다.

신호등을 켜려면 CLI 훅을 배선해야 합니다. **[docs/setup.md](docs/setup.md)** 를 보세요.

### 브라우저로 열기

WSL, 원격 머신처럼 터미널 에뮬레이터를 띄우기 번거로운 환경에서는 HTTP 로 접근할 수 있습니다:

```sh
sh scripts/polycanv-web.sh      # 서버 시작, 로그인 토큰과 주소를 알려줍니다
```

서버는 `127.0.0.1` 에만 바인딩합니다. 네트워크에 여는 것은 **터미널 접근 권한을 그 네트워크에
여는 일**입니다 — HTTPS 와 토큰 설정 없이는 하지 마세요.

## 구성

```
layouts/            캔버스 · 리스트 레이아웃
crates/protocol/    세션 상태 · 메타데이터 공유 계약
plugins/sidebar/    목록 렌더링, 선택 → 메인 교체, 뷰 전환
plugins/launcher/   도구 선택 → 새 패인에서 실행
plugins/status/     상태 판별 → 신호등
scripts/            설치, CLI 훅 → 상태 브리지
```

## 백로그

남은 작업은 [이슈](https://github.com/maengyo/polycanv/issues)와
[프로젝트 보드](https://github.com/users/maengyo/projects/1)에 있습니다.
우선순위는 `P0`~`P3` 라벨이며 — **P0 은 처음 켠 사용자가 막히는 것**입니다.

## 상태

핵심 동작은 실측으로 확인됐습니다 — 뷰 전환 시 세션 보존, 접기·펼치기, 선택 → 메인 교체,
AI CLI 4종 동시 구동, 실제 claude 턴이 브리지를 거쳐 🔴 까지 도달.

확인되지 **않은** 것과 그 이유는 **[docs/setup.md](docs/setup.md) 8장**에 있습니다.
특히 **Windows / Linux 는 미검증**입니다 (개발이 macOS 에서 이뤄졌습니다).

설계 판단과 그 근거는 `CLAUDE.md`, 조사 원문은 `docs/research/`,
요청·결정·발견의 날짜별 기록은 `docs/worklog.md` 에 있습니다.
**뒤집힌 전제도 그대로 남겨두었습니다** — 왜 그렇게 결정했는지가 결정 자체보다 오래 갑니다.

## 라이선스

[MIT](LICENSE)
