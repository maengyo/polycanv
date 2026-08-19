# 작업 기록

요청받은 것, 결정한 것, 실측으로 알아낸 것을 날짜별로 남긴다.
**뒤집힌 판단도 지우지 않고 남긴다** — 왜 그렇게 결론냈는지가 결론 자체보다 오래 간다.

---

## 2026-08-18 (수) — 착수

### 요청
- 새 프로젝트를 위한 에이전트 팀 구성
- 목표: 여러 AI 코딩 CLI와 셸을 한 화면에 띄우고, 어느 세션이 끝났는지 한눈에 보는 TUI 도구

### 한 일
- `CLAUDE.md` 작성 — 절대 원칙 4개, 아키텍처, 파일 소유 경계, 미검증 리스크
- `.claude/agents/` 에 에이전트 6종 정의
  (zellij-scout / layout-view / launcher-plugin / status-detector / sidebar-ui / release-eng)
- 각 정의에 **소유 경계**와 "근거 없이 됐다고 하지 마라" 검증 규칙을 명시
- git 저장소 초기화, 디렉터리 골격 생성

### 결정
- 공유 계약 `crates/protocol/**` 은 **리드(메인 세션)만 수정**한다.
  병렬 작업 충돌은 대부분 공유 타입에서 터지기 때문.
- 이 Claude Code 빌드에서 `team_name` 기반 agent teams 는 deprecated →
  서브에이전트 정의 + 파일 소유 경계 + 병렬 dispatch 조합으로 구현

---

## 2026-08-19 (목) — 검증 · 구현 · 이름 확정

### 환경 구축
- 디스크가 **100% 차 있었음**(228GB 중 342MB) → 사용자 승인 후 캐시 정리로 5.4GB 확보
- zellij **0.44.3**, rustc/cargo **1.97.1**, wasm 타깃 **`wasm32-wasip1`** 설치
- 이후 qwen code(0.21.13), PowerShell(7.6.5), shellcheck 추가 설치

### [높음] 리스크 2건 해소

**리스크 1 — 패인 제어 API** (`docs/research/zellij-pane-control-api.md`)
- `replace_pane_with_existing_pane(밀려날, 올라올, suppress=true)` **한 번**이면 된다
- `false` 는 패인을 **닫는다** → 코드에 등장 금지
- 런타임 4회 왕복 검증 완료. upstream 의 `// TODO: test this` 경로도 통과
- 부수 발견: `override_layout` 은 기본값(`retain_*=false`)이 **호출자 자신의 패인까지 닫는다**

**리스크 2 — CLI 상태 훅** (`docs/research/cli-status-hooks.md`)
- claude / codex / opencode / qwen **4개 전부 ①계층(훅·이벤트)** 으로 확인
- codex 의 "훅 미발화"는 훅 부재가 아니라 **신뢰(trust) 게이트** 탓이었다.
  신뢰되지 않은 훅은 **오류도 경고도 없이 조용히 실행되지 않는다**

### 실측으로 뒤집힌 전제 (6건)

| 처음 판단 | 실측 결과 |
|---|---|
| swap layout 을 2개 두면 토글이 된다 | 기저가 `ExactPanes` 제약이라 순환 주기가 패인 수에 따라 변함 → **캔버스·리스트 둘 다 swap layout 으로** 선언해야 함 |
| codex 는 훅이 없어 패턴매칭이 필요하다 | 훅은 있고 **신뢰 게이트**가 막고 있었다 |
| 파이프가 매달리는 건 원인 불명 | **권한 미승인 인스턴스**에 보내면 unblock 이 거부되어 영구 대기 |
| 권한 부족은 우아하게 실패한다 | **플러그인이 패닉한다.** 값을 반환하는 호출은 `stdin EOF` 로 죽어 오류가 권한을 언급조차 안 함 |
| `hide_floating_panes` 로 플러그인을 숨긴다 | **레이아웃 옵션이 아니다.** 세션 저장용이고, zellij 는 모르는 옵션을 조용히 무시한다 |
| 플러그인 화면은 외부에서 볼 수 없다 | **PTY 를 직접 붙이면 볼 수 있다.** `debug_render` 로 프레임을 로그에 남겨 검증 |

### 구현
- `crates/protocol` — 상태·패인 공유 계약. 출처 등급 기반 병합, 확인 시 🔴 해제
- `plugins/sidebar` — 목록 렌더링, 선택 → 메인 교체, 2단계 뷰 전환
- `plugins/launcher` — 도구 선택 실행, **사이드카**(브리지 동반 실행) 지원
- `plugins/status` — opencode SSE 어댑터 (다른 세션 작업분 통합)
- `scripts/polycanv-hook.sh` — CLI 훅 → 사이드바 상태 브리지
- `scripts/install.sh`, 3-OS 매트릭스 CI, README, `docs/setup.md`

### 통합 중 잡은 결함
- status 플러그인이 **cdylib 으로만 빌드**돼 있어 한 번도 로드된 적이 없었다
  (zellij 는 바이너리 크레이트를 요구 — 빌드는 성공하고 로드만 실패한다)
- 바이너리로 바꾼 뒤에도 **49KB 빈 wasm** 이 나옴 → `extern crate` 참조로 링크 강제 (1.7MB)
- 사이드바가 상태를 `AgentState` 로만 저장 → **🔴 이 확인해도 해제되지 않았다.**
  `StatusRecord` 로 바꿔 병합·확인 규칙 연결
- 사이드바가 타이머마다 **모든 패인**에 메타데이터를 조회 → 100ms 타임아웃에 걸려
  `GetPaneRunningCommand timed out` **44,720회**. 틱당 최대 2개로 제한

### 사용자 요청으로 풀린 것
- **"멀티 터미널이 목표이니 qwen 인증은 필요 없다"** → 인증 없이 훅 검증 성공.
  이 관점이 "CLI 인증은 polycanv 의 의존성이 아니다"라는 설계 원칙이 됨
- 같은 논리로 PowerShell 도 macOS 에 설치해 검증 → 5개 도구 동시 구동 확인

### 결정 (사용자)
- **프로젝트명: `polycanv`** (기존 `tercanv` 에서 변경, 409곳 일괄 치환)
- **라이선스: MIT** — Zellij 와 동일
- **저장소: public**
- 공개 전 **이메일·절대경로 전량 정리**
