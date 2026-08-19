# CLI별 상태 판별 훅/이벤트 조사

> 리스크 2 (높음) 검증 결과. 1차 조사 2026-08-19, **TUI 스파이크 반영 2026-08-19 (2차)**.
> 조사 대상: claude code 2.1.234, codex-cli 0.147.0, opencode 1.14.48, qwen code 0.21.13.
> 근거 없는 칸은 **미확인**으로 남겼다. 추측으로 채우지 않았다.

## 증거 등급 표기

문서 안의 모든 주장에 아래 등급을 붙였다. 등급 없는 서술은 배경 설명이다.

| 표기 | 뜻 |
|---|---|
| **[TUI실측]** | zellij 패인에서 인터랙티브 TUI를 띄우고 키를 주입해 직접 확인 — **가장 강한 증거** |
| **[헤드리스실측]** | `claude -p` / `codex exec` 등 비인터랙티브 실행으로 확인 |
| **[바이너리]** | 설치된 바이너리/번들 소스의 문자열·코드에서 확인 (문서보다 강함) |
| **[문서]** | 공식 문서에만 근거. 실행으로 확인하지 않음 |

2차 스파이크는 zellij 0.44.3 패인 + `zellij action write-chars` / `write` / `dump-screen` 조합으로
진행했다. 세션은 `polycanv-scout2-*` 3개를 썼고 전부 `zellij delete-session`으로 정리했다.

## 0. 결론 요약

| CLI | 최상위 수단 | running | waiting(승인대기) | finished | 판정 |
|---|---|---|---|---|---|
| **claude code** | ① 훅 | `UserPromptSubmit` | `PermissionRequest` + `Notification`(`permission_prompt`) | `Stop` | ✅ **[TUI실측] 완전 구분** |
| **codex cli** | ① 훅 | `UserPromptSubmit` | `PermissionRequest` | `Stop` | ✅ **[TUI실측] 완전 구분** |
| **opencode** | ① 이벤트(SSE) | `session.status{busy}` | `permission.updated` | `session.idle` | ✅ 구분 가능 (SSE 엔드포인트 실측) |
| **qwen code** | ① 훅 | `UserPromptSubmit` | `PermissionRequest` | `Stop` | ✅ **[TUI실측] 완전 구분** |

> ### 🎉 리스크 2 해소 — 4개 CLI 전부 ① 계층에서 waiting/finished가 갈린다.
>
> 1차 조사에서 codex만 `notify` + 출력패턴 하이브리드로 남았으나, **2차 TUI 스파이크에서
> codex 훅이 정상 발화하는 것을 확인했다.** 1차에서 실패한 원인은 설정 위치나 스키마가 아니라
> **codex의 훅 신뢰(hook trust) 게이트**였다 — 자세한 건 §2-1.
>
> **결과: ③ 출력 패턴매칭 코드는 전 CLI에서 불필요하다.** `plugins/status`는 훅/이벤트
> 수신만 구현하면 된다. 출력 패턴은 최후 폴백으로만 §부록에 남겨둔다.

### 상태 전이 시퀀스 (전부 [TUI실측])

```
claude code : SessionStart → UserPromptSubmit → PreToolUse → PermissionRequest → Notification → Stop
codex cli   : SessionStart → UserPromptSubmit → PreToolUse → PermissionRequest → PostToolUse  → Stop
qwen code   : SessionStart → UserPromptSubmit → PermissionRequest → PreToolUse  → …           → Stop
```

> ⚠️ **`PreToolUse`와 `PermissionRequest`의 순서가 CLI마다 다르다.**
> claude·codex는 `PreToolUse` → `PermissionRequest`, qwen은 **역순**이다.
> 따라서 "`PreToolUse`가 왔는데 `PostToolUse`가 안 오면 waiting" 같은 추론은 쓰면 안 된다.
> **`PermissionRequest` 수신 자체를 waiting 진입 신호로 삼고, `Stop`(또는 `PostToolUse`)을 해제 신호로 삼아라.**

---

## 1. claude code — ① 훅 (실측 검증 완료)

### 1-1. 훅 시스템

설정 파일: `~/.claude/settings.json` (user) / `.claude/settings.json` (project) / `--settings <path>` (CLI 플래그).

이 머신에 이미 전 이벤트가 설정돼 있다 — 근거 `~/.claude/settings.json`
(`SessionStart`, `SessionEnd`, `PreToolUse`, `PostToolUse`, `UserPromptSubmit`, `PermissionRequest`, `Notification`, `Stop`, `SubagentStart`, `SubagentStop`, `PreCompact` 전부 등록됨).

설정 형식:

```json
{
  "hooks": {
    "Stop": [
      { "hooks": [ { "type": "command", "command": "/path/to/script.sh", "timeout": 2 } ] }
    ],
    "PermissionRequest": [
      { "matcher": ".*", "hooks": [ { "type": "command", "command": "..." } ] }
    ]
  }
}
```

호출 방식: **command 훅에 stdin으로 JSON 1개**가 들어온다 (HTTP 훅은 POST body).

### 1-2. 실측 페이로드

검증 명령 (스크래치패드에서 실행):

```sh
claude -p "Run the bash command: echo HELLO_POLYCANV" \
  --settings $S/settings.json --allowedTools "Bash"
```

발화 순서 실측: `SessionStart` → `UserPromptSubmit` → `PreToolUse` → `Stop` → `SessionEnd`.

실제로 받은 페이로드(원문, session_id만 그대로):

```json
{"session_id":"726cde1a-35db-4688-9d4d-5ee4b3d46a7e",
 "transcript_path":"~/.claude/projects/.../726cde1a-....jsonl",
 "cwd":"/private/tmp/.../scratchpad",
 "hook_event_name":"SessionStart","source":"startup"}
```

```json
{"session_id":"726cde1a-...","prompt_id":"6da5ee5e-...","permission_mode":"default",
 "hook_event_name":"UserPromptSubmit",
 "prompt":"Run the bash command: echo HELLO_POLYCANV"}
```

```json
{"session_id":"726cde1a-...","prompt_id":"6da5ee5e-...","permission_mode":"default",
 "effort":{"level":"high"},"hook_event_name":"Stop","stop_hook_active":false,
 "last_assistant_message":"Output: `HELLO_POLYCANV`","background_tasks":...}
```

```json
{"session_id":"726cde1a-...","hook_event_name":"SessionEnd","reason":"other"}
```

> ⚠️ **문서와 실물의 차이**: 공식 문서는 `session_start_reason` / `session_end_reason`이라고 적었지만
> 실제 페이로드 키는 `source` / `reason`이다. 파서는 실측값 기준으로 짜야 한다.
> 근거: 위 실행 출력 vs <https://code.claude.com/docs/en/hooks>

### 1-3. waiting vs finished

| 상태 | 훅 | 판별 키 | 증거 |
|---|---|---|---|
| running | `UserPromptSubmit` | 발화 자체 | [TUI실측] |
| waiting | `PermissionRequest` | `tool_name` + `tool_input` + `permission_suggestions` | **[TUI실측]** |
| waiting(동시발화) | `Notification` | `notification_type == "permission_prompt"` | **[TUI실측]** |
| finished | `Stop` | `last_assistant_message` | [TUI실측] |

#### ✅ TUI 스파이크 결과 (2차) — 1차의 미검증 항목 해소

zellij 세션 `polycanv-scout2-claude`에서 `claude --settings <hooks> --permission-mode default`를
띄우고, 승인이 필요한 작업(파일 생성)을 시켜 다이얼로그를 실제로 띄웠다.

다이얼로그가 화면에 떠 있는 시점(`zellij action dump-screen`으로 확인)의 훅 발화 순서:

```
SessionStart → UserPromptSubmit → PreToolUse → PermissionRequest → Notification
```

→ **`PermissionRequest`와 `Notification`이 둘 다 발화했고 `Stop`은 아직 오지 않았다.**
승인 키(Enter)를 주입하자 `Stop`이 이어서 발화했다. **waiting과 finished가 명확히 갈린다.**

실제 수신한 `Notification` 페이로드 **[TUI실측]**:

```json
{"session_id":"33dfae8c-8edc-4415-8c67-10822e0e09b4",
 "transcript_path":"~/.claude/projects/.../33dfae8c-....jsonl",
 "cwd":".../scratchpad/cwork",
 "prompt_id":"69694991-f992-4d49-b530-1abfa37c1333",
 "hook_event_name":"Notification",
 "message":"Claude needs your permission",
 "notification_type":"permission_prompt"}
```

실제 수신한 `PermissionRequest` 페이로드 **[TUI실측]**:

```json
{"session_id":"33dfae8c-...","prompt_id":"69694991-...","permission_mode":"default",
 "effort":{"level":"high"},"hook_event_name":"PermissionRequest",
 "tool_name":"Write",
 "tool_input":{"file_path":".../cwork/note.txt","content":"hello\n"},
 "permission_suggestions":[{"type":"setMode","mode":"acceptEdits", ...}]}
```

화면에 뜬 다이얼로그 원문 **[TUI실측]**:

```
 Do you want to create note.txt?
 ❯ 1. Yes
   2. Yes, allow all edits during this session (shift+tab)
   3. No
 Esc to cancel · Tab to amend
```

> `notification_type`의 나머지 값(`idle_prompt`, `auth_success`, `agent_needs_input` 등)은
> 여전히 **[문서]** 근거뿐이다 (<https://code.claude.com/docs/en/hooks>).
> polycanv에 필요한 `permission_prompt`만 [TUI실측]으로 확정됐다.
>
> **헤드리스로는 원리상 검증 불가**였다는 1차 기록은 유효하다 — `claude -p`는 승인 다이얼로그를
> 띄우지 않고 자동 허용/거부하므로 `PermissionRequest`/`Notification`이 발화하지 않는다.
> 이 두 훅은 **TUI에서만** 관측된다.

### 1-4. ② 로그 파일 (백업 수단)

경로: `~/.claude/projects/<sanitized-cwd>/<session-uuid>.jsonl`
훅 페이로드의 `transcript_path`가 정확히 이 파일을 가리킨다(위 실측 참조).

실측한 레코드 `type` 값 (`grep -oE '"type":"[a-z_]+"'` 결과):
`user`, `assistant`, `message`, `text`, `tool_use`, `tool_result`, `attachment`,
`hook_success`, `hook_additional_context`, `skill_listing`, `total_tokens_reminder` 등.

> JSONL에는 **승인 대기 상태를 직접 표시하는 레코드가 없다.** finished는 마지막 레코드가
> assistant 메시지인지로 추론 가능하지만 waiting은 추론 불가 → ① 훅이 필수.

### 1-5. ③ 출력 패턴 (최후 수단)

`~/.local/share/claude/versions/2.1.234` (Mach-O) `strings` 추출 실측:

```
needs your permission to use
Do you want to proceed?
Do you want to allow Claude to fetch this content?
Do you want to allow this connection?
Yes, and don't ask again for
Yes, and allow access to
No, and tell Claude what to do differently
```

승인 대기 판별용 정규식 후보: `No, and tell Claude what to do differently` — 승인 다이얼로그에만
등장하므로 오탐이 가장 적다.

---

## 2. codex cli — ✅ 해결됨 (2차 TUI 스파이크)

> **1차 결론 뒤집힘.** 1차에서 "훅 미작동 → 출력패턴 필요"로 판정했으나, 2차 TUI 스파이크에서
> **훅이 정상 발화**하는 것을 확인했다. 원인은 아래 §2-1의 **훅 신뢰 게이트**였다.

### 2-1. 훅 시스템 — ✅ TUI에서 정상 발화

#### 왜 1차(`codex exec`)에서 실패했는가 — 훅 신뢰(hook trust) 게이트

`codex --help`에 다음 플래그가 있다 **[바이너리]**:

```
--dangerously-bypass-hook-trust
        Run enabled hooks without requiring persisted hook trust for this invocation. DANGEROUS.
```

즉 codex는 **훅을 실행하기 전에 사용자의 명시적 신뢰(trust)를 요구**하며, 그 신뢰는 디스크에 영속된다.
TUI로 띄우자 실제로 두 단계 게이트가 순서대로 떴다 **[TUI실측]**:

1단계 — 디렉터리 신뢰:

```
 Do you trust the contents of this directory? Working with untrusted contents comes with higher
 risk of prompt injection. Trusting the directory allows project-local config, hooks, and exec
 policies to load.
 › 1. Yes, continue
   2. No, quit
```

2단계 — 훅 신뢰:

```
 Hooks need review
 7 hooks are new or changed.
 Hooks can run outside the sandbox after you trust them.
 › 1. Review hooks
   2. Trust all and continue
   3. Continue without trusting (hooks won't run)
```

> **`codex exec`(비인터랙티브)에는 이 프롬프트를 띄울 방법이 없다.** 그래서 1차에서 시도한 5가지
> 설정 배치가 전부 조용히 무시된 것이다 — 설정 위치도, snake_case/PascalCase 스키마도 문제가 아니었다.
> `hooks.json`의 PascalCase 이벤트 키가 **정답**이었다.
>
> 신뢰는 **영속된다** — 세션을 지우고 재기동해도 두 프롬프트가 다시 뜨지 않았다 **[TUI실측]**.
> 즉 polycanv는 **최초 1회만** 사용자가 신뢰를 승인하면 이후 자동으로 훅이 동작한다.

#### 검증된 설정 (그대로 쓰면 된다)

경로: `$CODEX_HOME/hooks.json` (기본 `~/.codex/hooks.json`). **이벤트 키는 PascalCase.**

```json
{
  "hooks": {
    "SessionStart":      [{"hooks":[{"type":"command","command":"/path/hook.sh SessionStart"}]}],
    "UserPromptSubmit":  [{"hooks":[{"type":"command","command":"/path/hook.sh UserPromptSubmit"}]}],
    "PreToolUse":        [{"matcher":".*","hooks":[{"type":"command","command":"/path/hook.sh PreToolUse"}]}],
    "PermissionRequest": [{"matcher":".*","hooks":[{"type":"command","command":"/path/hook.sh PermissionRequest"}]}],
    "PostToolUse":       [{"matcher":".*","hooks":[{"type":"command","command":"/path/hook.sh PostToolUse"}]}],
    "Stop":              [{"hooks":[{"type":"command","command":"/path/hook.sh Stop"}]}],
    "SessionEnd":        [{"hooks":[{"type":"command","command":"/path/hook.sh SessionEnd"}]}]
  }
}
```

기능 플래그는 기본 활성 — `codex features list` **[실측]**:

```
hooks                                stable             true
```

호출 방식: claude code와 동일하게 **stdin으로 JSON 1개**.

### 2-1b. waiting vs finished — ✅ 훅으로 완전 구분 [TUI실측]

zellij 세션 `polycanv-scout2-hooks`에서 `codex -a untrusted`로 띄우고 승인이 필요한 명령을 시켰다.

승인 다이얼로그가 화면에 떠 있는 시점의 훅 발화 순서:

```
SessionStart → UserPromptSubmit → PreToolUse → PermissionRequest
```

→ **`PermissionRequest`까지 오고 `Stop`은 없다 = waiting.**
`y`를 주입해 승인하자 `PostToolUse` → `Stop`이 이어졌다 = finished.

| 상태 | 훅 | 증거 |
|---|---|---|
| running | `UserPromptSubmit` | [TUI실측] |
| waiting | `PermissionRequest` | **[TUI실측]** |
| finished | `Stop` | [TUI실측] |

실제 수신한 `PermissionRequest` 페이로드 **[TUI실측]**:

```json
{"session_id":"01a01597-78da-7ab3-8588-7f864b6b3729",
 "turn_id":"01a01597-fab7-7f73-af8e-077aaaedb175",
 "transcript_path":".../codexhome/sessions/2026/08/19/rollout-2026-08-19T00-57-23-01a01597-....jsonl",
 "cwd":".../scratchpad/work",
 "hook_event_name":"PermissionRequest","model":"gpt-5.5","permission_mode":"default",
 "tool_name":"Bash",
 "tool_input":{"command":"rm -f /private/tmp/polycanv-outside-test.txt"}}
```

실제 수신한 `SessionStart` / `PreToolUse` / `Stop` 페이로드 **[TUI실측]** (요약):

```json
{"session_id":"01a01596-...","hook_event_name":"SessionStart",
 "model":"gpt-5.5","permission_mode":"default","source":"startup",
 "transcript_path":".../rollout-....jsonl","cwd":".../work"}
```
```json
{"session_id":"01a01596-...","turn_id":"01a01596-866e-71a3-a41c-003dbd9be09a",
 "hook_event_name":"PreToolUse","tool_name":"Bash",
 "tool_input":{"command":"echo hi > /private/tmp/polycanv-outside-test.txt"},
 "tool_use_id":"call_U3v28HVAiGOJ56tMUHxjzLZw"}
```
```json
{"session_id":"01a01596-...","turn_id":"01a01596-...","hook_event_name":"Stop",
 "stop_hook_active":false,"last_assistant_message":null}
```

> ⚠️ **codex의 `Stop.last_assistant_message`는 `null`일 수 있다** (위 실측). 이 필드로 finished를
> 판정하지 말고 **`Stop` 이벤트 수신 자체**로 판정해야 한다. claude code는 문자열이 채워져 있었다.
>
> ⚠️ **`turn_id`는 turn 스코프 훅에만 있고 `SessionStart`에는 없다** (위 실측).

승인 다이얼로그 화면 원문 **[TUI실측]**:

```
  Would you like to run the following command?
  Environment: local
  $ rm -f /private/tmp/polycanv-outside-test.txt
› 1. Yes, proceed (y)
  2. Yes, and don't ask again for commands that start with `rm -f ...` (p)
  3. No, and tell Codex what to do differently (esc)
  Press enter to confirm or esc to cancel
```

### 2-2. `notify` — ✅ 실측 성공 (단, finished 전용)

`~/.codex/config.toml` 최상단(테이블 앞)에:

```toml
notify = ["/path/to/notifier.sh"]
```

codex가 마지막 인자로 JSON 1개를 붙여 argv로 실행한다. **실측 수신 페이로드**:

```json
{"type":"agent-turn-complete",
 "thread-id":"01a01579-5f8a-7291-9e13-131260321e1c",
 "turn-id":"01a01579-6035-71a2-80e4-dad7cdb8199b",
 "cwd":"/private/tmp/.../scratchpad",
 "client":"codex_exec",
 "input-messages":["reply with exactly: OK5"],
 "last-assistant-message":"OK5"}
```

> 이벤트 타입은 **`agent-turn-complete` 하나뿐이다.** 바이너리 `strings`에서도
> `agent-turn-complete` / `input-messages` / `last-assistant-message` 3개만 검출됐고
> 승인 관련 notify 타입은 없다.
> 참고: 이 머신의 실제 `~/.codex/config.toml`은 `notify = [".../SkyComputerUseClient", "turn-ended"]`인데
> `"turn-ended"`는 **이벤트 이름이 아니라 그 프로그램에 넘기는 argv 토큰**이다. 혼동 주의.
>
> 바이너리에 `legacy_notify` 문자열이 있는 것으로 보아 notify는 훅으로 대체되는 과정의 레거시 경로다.

**결론: notify로 finished는 잡히지만 waiting은 잡히지 않는다.**

### 2-3. ② 로그/세션 파일

경로: `~/.codex/sessions/YYYY/MM/DD/rollout-<ISO8601>-<uuid>.jsonl`
(실측 예: `~/.codex/sessions/2026/05/17/rollout-2026-05-17T15-55-48-019e34b8-....jsonl`)

`event_msg` 레코드의 `payload.type` 실측 분포 (2026/05 전체 롤아웃 집계):

```
1821 token_count      1304 function_call        1038 exec_command_end
 480 agent_message     109 task_started          102 task_complete
   7 turn_aborted        7 context_compacted
```

`task_complete` 실측 원문:

```json
{"timestamp":"2026-05-17T07:03:14.976Z","type":"event_msg",
 "payload":{"type":"task_complete","turn_id":"019e34bc-...",
 "last_agent_message":"...","completed_at":1779001394,
 "duration_ms":140654,"time_to_first_token_ms":14257}}
```

매핑: `task_started` → running, `task_complete` → finished, `turn_aborted` → 중단.

> ❌ **승인 대기는 롤아웃 JSONL에 기록되지 않는다.**
> 바이너리에는 `exec_approval_request` / `apply_patch_approval_request` 문자열이 존재하지만
> (각 8회 검출), 사용자 롤아웃 파일 전체를 `grep -l 'approval_request'` 해도 **0건**이다.
> 즉 승인 요청은 프로토콜 내부 이벤트일 뿐 디스크에 남지 않는다.

### 2-4. ③ 출력 패턴 — **waiting 판별의 현재 유일한 수단**

codex 바이너리 `strings` 실측 (TUI 승인 다이얼로그 문자열):

```
Approve app tool call?
Do you want to approve network access to
Yes, proceed
Yes, and allow these permissions for this session
Yes, and allow this host in the future
No, and tell Codex what to do differently
```

권장 정규식: `No, and tell Codex what to do differently` 또는 `^\s*Yes, proceed`
— 승인 다이얼로그 외에는 등장하지 않는다.

진행중 표시: `Working`, `Thinking` 문자열 검출됨 (단, 일반 텍스트에도 나올 수 있어 오탐 위험).

### 2-5. 벨 문자

`strings`에서 `BEL` 2건 검출. 다만 **어느 이벤트에 붙는지 확인하지 못했다 → 미확인.**

### 2-6. 그 외 통로 (참고)

`codex app-server` (JSON-RPC, experimental) / `codex mcp-server` (stdio)는
`exec_approval_request` 등 승인 이벤트를 클라이언트에 넘기는 구조로 보이나,
polycanv는 Zellij 패인에서 **인터랙티브 TUI**를 띄우는 설계이므로 적용 불가.
`codex --remote <ADDR>` 로 TUI를 원격 app-server에 붙이는 경로가 있지만 **미확인**.

---

## 3. opencode — ① 이벤트 (실측 검증 완료)

### 3-1. SSE 이벤트 스트림

opencode는 **항상 HTTP 서버를 띄운다.** TUI도 내부 서버를 갖고 랜덤 포트에 바인딩한다
(근거: <https://opencode.ai/docs/server/> — "when you start the TUI, it randomly assigns a port and hostname").

실측 검증:

```sh
opencode serve --port 47311 --hostname 127.0.0.1
# → "opencode server listening on http://127.0.0.1:47311"

curl -sN http://127.0.0.1:47311/event
# → data: {"id":"evt_0157723140011Kg8grmTqjYfQq","type":"server.connected","properties":{}}
```

OpenAPI 스펙(`GET /doc`)에서 실측 확인된 이벤트 이름:
`session.idle`, `session.status`, `permission.replied`, `session.error`.

### 3-2. 이벤트 페이로드 (로컬 SDK 타입 원문)

근거: `~/.config/opencode/node_modules/@opencode-ai/sdk/dist/gen/types.gen.d.ts`

```ts
export type SessionStatus =
  | { type: "idle" }
  | { type: "retry"; attempt: number; message: string; next: number }
  | { type: "busy" };

export type EventSessionStatus = {
  type: "session.status";
  properties: { sessionID: string; status: SessionStatus };
};

export type EventSessionIdle = {
  type: "session.idle";
  properties: { sessionID: string };
};

export type Permission = {
  id: string; type: string; pattern?: string | string[];
  sessionID: string; messageID: string; callID?: string;
  title: string; metadata: Record<string, unknown>;
  time: { created: number };
};

export type EventPermissionUpdated = {
  type: "permission.updated";
  properties: Permission;           // ← 승인 대기 발생
};

export type EventPermissionReplied = {
  type: "permission.replied";
  properties: { sessionID: string; permissionID: string; response: string };
                                     // ← 승인 대기 해소
};
```

전체 이벤트 유니온 (`Event` 타입, 같은 파일 602행)에는 위 외에
`message.updated`, `message.part.updated`, `session.created`, `session.compacted`,
`file.edited`, `todo.updated`, `pty.*`, `server.connected` 등이 포함된다.

### 3-3. waiting vs finished — 완벽히 구분됨

| 상태 | 이벤트 |
|---|---|
| running | `session.status` + `properties.status.type === "busy"` |
| waiting | `permission.updated` 수신 후 `permission.replied` 미수신 |
| finished | `session.idle` 또는 `session.status` + `status.type === "idle"` |
| 재시도중 | `session.status` + `status.type === "retry"` (attempt/next 포함) |

**4개 CLI 중 상태 모델이 polycanv 신호등과 가장 정확히 일치한다.**

### 3-4. 플러그인 훅 (대안 경로)

근거: `~/.config/opencode/node_modules/@opencode-ai/plugin/dist/index.d.ts`

```ts
export interface Hooks {
  event?: (input: { event: Event }) => Promise<void>;          // 전체 이벤트 수신
  "permission.ask"?: (input: Permission,
                      output: { status: "ask" | "deny" | "allow" }) => Promise<void>;
  "chat.message"?: (input: {...}, output: {...}) => Promise<void>;
  "tool.execute.before"?: ...; "tool.execute.after"?: ...;
}
```

플러그인 설정: `~/.config/opencode/opencode.json`의 `plugin: []` 배열, 또는 `opencode plugin <module>`.

### 3-5. 포트 탐색 (실측)

TUI는 포트를 랜덤 배정하고 파일로 노출하지 않는다 — `~/.local/share/opencode/` 아래
포트/락 파일을 찾았으나 **없다**(`opencode.db`, `log/`, `storage/`, `repos/`만 존재).

실측 가능한 우회: 프로세스에서 직접 읽는다.

```sh
lsof -nP -iTCP -sTCP:LISTEN | grep opencode
# opencode 10467 user  9u IPv4 ... TCP 127.0.0.1:47311 (LISTEN)
```

> **polycanv 권장**: 런처가 opencode를 띄울 때 `--port`를 직접 지정해서
> 포트 탐색 자체를 없애는 게 가장 견고하다 (`opencode --port <할당포트>`).

---

## 4. qwen code — 문서 기반 (⚠️ 로컬 미설치, 실측 없음)

`which qwen` → `command not found`. 아래는 **전부 공식 문서 근거이며 실행으로 검증하지 않았다.**

### 4-1. 훅 시스템

Claude Code 스타일 훅을 그대로 채택했다.
근거: <https://qwenlm.github.io/qwen-code-docs/en/users/features/hooks/>
및 <https://github.com/QwenLM/qwen-code/blob/main/docs/users/features/hooks.md>

설정 파일: `.qwen/settings.json` (프로젝트) 및 사용자 설정 파일.
> 사용자 레벨 정확 경로(`~/.qwen/settings.json` 추정)는 문서가 명시하지 않았다 → **미확인**.

설정 형식 (문서 원문):

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "^run_shell_command$",
        "sequential": false,
        "hooks": [
          { "type": "command", "command": "/path/to/security-check.sh",
            "name": "security-check", "timeout": 30000 }
        ]
      }
    ],
    "SessionStart": [
      { "hooks": [ { "type": "command", "command": "echo 'Session started'" } ] }
    ]
  }
}
```

핸들러 타입 4종: `command`(stdin/stdout), `http`(JSON POST), `function`(내부), `prompt`(LLM 평가).
`http` 핸들러는 polycanv 입장에서 특히 편하다 — 스크립트 없이 바로 수신 가능.

### 4-2. waiting vs finished (문서상 구분 가능)

| 상태 | 이벤트 | 페이로드 필드 |
|---|---|---|
| running | `UserPromptSubmit` | `prompt`, `submitted_prompt` |
| waiting | `Notification` | `notification_type` ∈ `permission_prompt`, `idle_prompt`, `auth_success` |
| waiting | `PermissionRequest` | `permission_mode`, `tool_name`, `tool_input`, `permission_suggestions` |
| finished | `Stop` | `stop_hook_active`, `last_assistant_message`, `context_usage`, `context_limit`, `input_tokens` |

그 외 이벤트: `PostToolUse`, `PostToolUseFailure`, `SessionStart`(`source`), `SessionEnd`(`reason`),
`SubagentStart/Stop`, `PreCompact/PostCompact`, `TodoCreated/TodoCompleted`,
`MessageDisplay`, `StopFailure`.

> 문서 주석: `"elicitation_dialog"` 타입은 정의만 되어 있고 미구현.

### 4-3. ② 로그 파일

세션 히스토리: `~/.qwen/projects/<sanitized-cwd>/chats` (프로젝트 스코프 JSONL).
근거: <https://qwenlm.github.io/qwen-code-docs/en/users/features/headless/>
**형식 상세 미확인.**

### 4-4. 스트리밍 출력 (대안)

`qwen --output-format stream-json` — 줄단위 JSON. 이벤트: `system`(`subtype:"session_start"`),
`assistant`, `result`(`subtype:"success"`), `stream_event`(`--include-partial-messages` 시
`message_start` / `content_block_delta` / `goal_state`).
> 헤드리스 전용이라 TUI 패인 방식인 polycanv에는 부적합.

### 4-5. ③ 출력 패턴

**미확인** — 로컬 미설치라 바이너리 문자열을 추출하지 못했다.

---

## 5. 3계층 종합 표

| | claude code | codex cli | opencode | qwen code |
|---|---|---|---|---|
| **① 훅/이벤트** | ✅ `~/.claude/settings.json` — `Stop`/`Notification`/`PermissionRequest`. **stdin JSON 실측 완료** | ⚠️ 존재 확인(`features list`: `hooks stable true`, 바이너리 문자열) 그러나 **`codex exec`에서 5가지 배치 전부 미발화**. TUI 재검증 필요 | ✅ HTTP SSE `GET /event` — `session.status`/`session.idle`/`permission.updated`/`permission.replied`. **엔드포인트 실측 완료** | ⚠️ 문서상 존재 (`.qwen/settings.json`, Claude 호환 이벤트). **실측 없음** |
| **①.5 notify** | — | ✅ `config.toml`의 `notify=[...]` → argv 마지막에 JSON. **`agent-turn-complete` 페이로드 실측 완료**. finished 전용, waiting 불가 | — | — |
| **② 로그/세션 파일** | ✅ `~/.claude/projects/<cwd>/<uuid>.jsonl` (훅의 `transcript_path`와 동일). waiting 판별 불가 | ✅ `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl` — `task_started`/`task_complete`/`turn_aborted` 실측. **approval 레코드 0건 → waiting 불가** | (DB `~/.local/share/opencode/opencode.db` 존재하나 SSE가 상위 수단이라 미조사) | ⚠️ `~/.qwen/projects/<cwd>/chats` (문서만, 형식 미확인) |
| **③ 출력 패턴** | ✅ `No, and tell Claude what to do differently` / `needs your permission to use` / `Do you want to proceed?` (바이너리 strings 실측) | ✅ `No, and tell Codex what to do differently` / `Yes, proceed` / `Approve app tool call?` (바이너리 strings 실측) | 불필요 | ❌ **미확인** (미설치) |
| **벨 문자 `\a`** | 미확인 | `BEL` 문자열 2건 검출, **발화 조건 미확인** | 미확인 | 미확인 |

---

## 6. polycanv 구현 권고

1. **claude code / opencode / qwen code는 ① 계층으로 확정한다.** waiting/finished가 명확히 갈린다.
   - claude·qwen: 훅 스크립트가 상태를 어딘가로 흘려보내야 한다.
     qwen은 `http` 핸들러가 있으니 두 CLI 모두 **작은 로컬 HTTP 수신기 하나**로 통일하는 게 깔끔하다.
     (claude는 `command` 훅 + curl, 또는 문서상 HTTP 훅도 지원)
   - opencode: 런처가 `--port`를 명시 지정 → SSE `/event` 구독. 스크립트 주입 불필요.

2. **codex만 하이브리드다.** finished는 `notify`(실측 완료), waiting은 **③ 출력 패턴**.
   `crates/protocol`의 상태 이벤트 타입은 **소스가 훅이든 패턴매칭이든 동일하게 표현**되도록
   설계해야 한다 (예: `StatusEvent { source: Hook | Notify | Pattern, state: ... }`).

3. **착수 전 스파이크 2건** (둘 다 인터랙티브 TUI에서):
   - **[필수]** codex TUI에서 `~/.codex/hooks.json`이 발화하는가.
     발화하면 codex도 ① 계층으로 승격되고 ③ 패턴매칭 코드가 통째로 불필요해진다.
   - **[권장]** claude code TUI에서 `Notification`(`notification_type=permission_prompt`)이
     실제로 발화하는가. 헤드리스에서는 원리상 검증 불가했다.

4. **`SessionStart` 페이로드 키는 `source`, `SessionEnd`는 `reason`이다** — 공식 문서 표기와 다르다.
   파서는 실측 기준으로 작성할 것.

---

## 7. 부록 — 재현 명령

```sh
# 설치 확인
which claude codex opencode qwen

# claude 훅 실측
claude -p "Run the bash command: echo HELLO" --settings ./settings.json --allowedTools "Bash"

# codex notify 실측 (config.toml 최상단에 notify = ["/path/to/notifier.sh"])
CODEX_HOME=$CH codex exec --skip-git-repo-check "reply with exactly: OK" < /dev/null
#   ↑ `< /dev/null` 필수. 없으면 "Reading additional input from stdin..."에서 무한 대기한다.

# codex 기능 플래그
codex features list | grep hooks

# codex 바이너리 문자열 (경로는 brew 설치 기준)
strings -a /opt/homebrew/lib/node_modules/@openai/codex/node_modules/@openai/\
codex-darwin-arm64/vendor/aarch64-apple-darwin/bin/codex | grep -oE 'agent-turn-complete'

# opencode SSE 실측
opencode serve --port 47311 --hostname 127.0.0.1 &
curl -sN http://127.0.0.1:47311/event
curl -s http://127.0.0.1:47311/doc | grep -oE '"session\.(idle|status)"'
lsof -nP -iTCP -sTCP:LISTEN | grep opencode   # 실행중 TUI의 포트 탐색
```

## 8. 참고 URL

- Claude Code 훅 레퍼런스 — <https://code.claude.com/docs/en/hooks>
- Codex 훅 문서 — <https://learn.chatgpt.com/docs/hooks> (`developers.openai.com/codex/hooks`에서 308)
- opencode 서버 — <https://opencode.ai/docs/server/>
- Qwen Code 훅 — <https://qwenlm.github.io/qwen-code-docs/en/users/features/hooks/>
- Qwen Code 헤드리스 — <https://qwenlm.github.io/qwen-code-docs/en/users/features/headless/>
- 선례: sverrirsig/claude-control — <https://github.com/sverrirsig/claude-control>
  (Working/Idle/Waiting/Errored/Finished 분류. 훅 이벤트로 PID↔JSONL 매핑, mtime 폴백 + `status-classifier.ts`)
- 선례: Stargx/claude-code-dashboard — <https://github.com/Stargx/claude-code-dashboard>
  (`~/.claude/projects/` JSONL 증분 파싱)

---

# 9. codex 훅 — TUI 실측 (리드 직접 수행, 2026-08-19)

## 결론: **codex 는 ①계층으로 승격된다. 출력 패턴매칭은 불필요하다.**

1~8장의 "codex 는 훅이 미발화하므로 하이브리드" 결론은 **원인이 밝혀지면서 뒤집혔다.**
미발화의 원인은 훅 부재가 아니라 **훅 신뢰(trust) 게이트**였다.

| 이벤트 | 발화 | 검증 경로 | polycanv 매핑 |
|---|---|---|---|
| `SessionStart` | ✅ | 헤드리스 + TUI | — |
| `UserPromptSubmit` | ✅ | 헤드리스 + TUI | 🟢 running |
| `PreToolUse` | ✅ | 헤드리스 + TUI | 🟢 running (유지) |
| **`PermissionRequest`** | ✅ | **TUI 전용** | **🟡 waiting** |
| `Stop` | ✅ | 헤드리스 | 🔴 finished |

**`PermissionRequest` 는 승인 프롬프트가 화면에 뜨는 바로 그 시점에 발화한다.**
이것으로 codex 도 waiting/finished 가 훅으로 완전히 갈린다.

## 왜 1차 조사에서 미발화했는가 — 신뢰 게이트

codex 0.147.0 은 훅에 **지속되는 신뢰(persisted trust)** 를 요구한다. 신뢰되지 않은 훅은
설정에 있어도 **조용히 실행되지 않는다** (오류도 경고도 없다 — 그래서 1차에서 원인을 못 찾았다).

- TUI 에는 훅 관리 화면이 있다: `New hook - review required` / `Trusted` /
  `Modified since last trusted - review required`, "1 hook needs review before it can run."
- 신뢰를 우회하는 수단: `--dangerously-bypass-hook-trust` (또는 config `bypass_hook_trust`).
  도움말: *"Run enabled hooks without requiring persisted hook trust for this invocation."*

**실측 대조 (동일 설정, 플래그만 차이):**
```
$ CODEX_HOME=$H codex exec --sandbox read-only --skip-git-repo-check "Say exactly PING"
>>> 훅 발화: 0

$ CODEX_HOME=$H codex exec --sandbox read-only --skip-git-repo-check \
      --dangerously-bypass-hook-trust "Say exactly PING"
>>> 훅 발화: 3   (SessionStart / UserPromptSubmit / Stop)
```

## 설정 스키마 — `hooks` 는 **파일 경로가 아니라 config.toml 안의 테이블**이다

1차 문서가 참조한 `hooks.json` 경로 방식은 **플러그인 매니페스트용**이다.
사용자 설정에서 `hooks = "hooks.json"` 로 쓰면 다음 오류가 난다:
```
Error loading config.toml: invalid type: string "hooks.json", expected struct HooksToml in `hooks`
```

맞는 형식 (`$CODEX_HOME/config.toml`):
```toml
[[hooks.user_prompt_submit]]
type = "command"
command = "/path/to/hook.sh"

[[hooks.permission_request]]
type = "command"
command = "/path/to/hook.sh"

[[hooks.stop]]
type = "command"
command = "/path/to/hook.sh"
```
이벤트 키는 **snake_case** 다: `pre_tool_use` `permission_request` `post_tool_use`
`pre_compact` `post_compact` `session_start` `session_end` `user_prompt_submit`
`subagent_start` `subagent_stop` `stop`.
페이로드의 `hook_event_name` 은 **PascalCase** 로 온다 (`PermissionRequest` 등). 파서 주의.

## `PermissionRequest` 실측 페이로드 (stdin JSON)

```json
{
  "session_id": "01a01714-f034-76f1-9330-a7ed0aa699d0",
  "turn_id": "01a01715-4b6e-78d1-b2b6-e103f4858cee",
  "transcript_path": ".../sessions/2026/08/19/rollout-...jsonl",
  "cwd": "/private/tmp/.../scratchpad",
  "hook_event_name": "PermissionRequest",
  "model": "gpt-5.6-sol",
  "permission_mode": "default",
  "tool_name": "Bash",
  "tool_input": {
    "command": "echo hello > .../approval-probe.txt",
    "description": "Do you want to allow this exact command to write approval-probe.txt in the workspace?"
  }
}
```
**`cwd` 가 페이로드에 들어 있다** — 사이드바 아이템의 작업 디렉터리를 별도 수단 없이 채울 수 있다.

## 재현 절차

TUI 검증은 zellij 패인 안에서 했다 (진짜 TTY 가 필요하다 — `codex exec` 에는 승인 흐름 자체가 없어
`PermissionRequest` 를 헤드리스로는 검증할 수 없다).

```
# 격리된 CODEX_HOME (사용자 실제 ~/.codex 를 건드리지 않는다. auth.json 만 심볼릭 링크)
$ ln -s ~/.codex/auth.json $H/auth.json
$ <위 config.toml 작성>

# zellij 패인에서 TUI 기동 → 밖에서 키 주입
$ zellij -s <세션> action new-pane --cwd $SP -- \
    env CODEX_HOME=$H codex --dangerously-bypass-hook-trust --sandbox read-only \
    -c approval_policy='"on-request"'
$ zellij -s <세션> action write-chars "<프롬프트>"; zellij -s <세션> action write 13
$ zellij -s <세션> action dump-screen -p terminal_N     # 화면 확인
```

## polycanv 에 미치는 영향

1. **4개 CLI 전부 ①계층이다.** `StatusSource::Pattern` 은 계약에 남겨두되(설정으로 추가되는
   미지의 도구용), **기본 제공 4종에는 쓰지 않는다.** codex 전용 패턴매칭 코드는 만들지 마라.
2. **런처가 훅을 심어야 한다.** codex 패인을 띄울 때 polycanv 전용 `CODEX_HOME` 을 쓰거나
   사용자 config 에 훅을 추가하는 설계가 필요하다. 후자는 사용자 설정을 건드리므로 전자가 낫다.
   단 전자는 `auth.json` 등 기존 상태를 물려줘야 한다.
3. ⚠️ **신뢰 게이트를 어떻게 넘길지 결정해야 한다.** `--dangerously-bypass-hook-trust` 는 이름 그대로
   위험하고, 사용자에게 그 플래그로 codex 를 띄우게 하는 것은 보안 결정을 대신하는 것이다.
   **대안: TUI 훅 화면에서 한 번 신뢰시키면 지속된다** (`hooks.state` 에 저장). 최초 1회 안내가
   더 정직한 설계로 보인다. **미결정 — 리드/사용자 판단 필요.**
4. `Stop` 은 헤드리스에서만 확인했다. TUI 에서도 발화하는지는 미검증이나, 다른 4개가 TUI 에서
   모두 발화했으므로 문제 없을 가능성이 높다. **추정: TUI 에서도 발화한다.**

---

# 10. qwen code — 설치 후 실측 (리드, 2026-08-19)

1~8장은 qwen 을 **미설치 상태에서 문서만으로** 다뤘다. 설치해서 확인한 결과를 덧붙인다.

설치: `npm i -g @qwen-code/qwen-code` → **0.21.13** (89MB). `/opt/homebrew/bin/qwen`

## 확인된 것

- **훅 시스템이 실재한다.** CLI 에 전용 하위 명령이 있다:
  `qwen hooks` — *"Manage Qwen Code hooks (use /hooks in interactive mode)"*
  → 1차 문서의 "문서상 존재" 가 **실물로 확인**됐다.
- `--safe-mode` 플래그가 *"Disable all customizations (context files, hooks, ...)"* 로
  훅을 끄는 스위치를 제공한다 — 훅이 정식 기능이라는 방증이다.
- **polycanv 패인 안에서 정상 구동된다.** claude / codex / opencode 와 **동시 실행** 확인.

## 확인하지 못한 것 — 자격 증명이 없다

훅이 **실제로 발화하는지는 검증하지 못했다.** 턴을 돌릴 수 없기 때문이다:

```
$ qwen -p "Reply with exactly: PONG"
No auth type is selected. Please configure an auth type
(e.g. via settings or `--auth-type`) before running in non-interactive mode.
```

`qwen auth` 하위 명령은 *(removed)* 로 표시돼 있어, 설정 파일이나 `--auth-type` 으로 지정해야 한다.

→ **막힌 지점은 기술이 아니라 계정이다.** 사용자가 qwen 인증을 설정하면
   claude 에 쓴 것과 **동일한 절차**(패인 안에서 한 턴 → 훅 로그 확인 → 브리지 통과)로
   몇 분 안에 검증할 수 있다. 브리지는 이벤트 이름 기준으로 동작하므로,
   qwen 이 claude 호환 이벤트(`UserPromptSubmit`/`Stop`)를 보내면 **코드 변경 없이 그대로 붙는다.**

**추정 (미검증)**: 1차 조사의 문서 기준으로 qwen 은 claude 호환 훅을 쓴다. 그렇다면
`scripts/polycanv-hook.sh` 가 그대로 동작한다. 확인 전까지 이것은 추정이다.

## 10-1. qwen 훅 실측 (인증 없이, 2026-08-19)

**설계 원칙**: polycanv 는 CLI 가 **인증돼 있을 것을 요구하지 않는다.** 하는 일은 터미널을 띄우고
훅을 읽는 것이고, 그 CLI에 로그인했는지는 사용자 사정이다. 그래서 인증 없이도 검증할 수 있다.

zellij 패인에서 qwen 을 띄운 결과 (프로젝트 로컬 `.qwen/settings.json` 으로 훅 지정):

```
qwen SessionStart (pane=31)
  → {"pane":{"terminal":31},"state":"idle","source":"hook","at_ms":...}
```

**확인된 것:**
- **훅이 실제로 발화한다.** 인증 없이, 프로바이더 미연결 상태에서도 `SessionStart` 가 온다.
- **이벤트 이름이 claude 호환**이다 (PascalCase `SessionStart`).
- **페이로드 키가 claude 와 같다**: `cwd` `hook_event_name` `model` `permission_mode`
  `session_id` `source` `timestamp` `transcript_path`.
  → `scripts/polycanv-hook.sh` 가 **코드 변경 없이 그대로 동작한다.** 실측으로 확인.
- 프로젝트 로컬 `.qwen/settings.json` 이 먹는다 — 사용자 홈 설정을 건드릴 필요가 없다.

**확인하지 못한 것**: `UserPromptSubmit` / `Stop`. 프로바이더가 연결되지 않아 턴이 시작되지 않는다
(TUI 가 "Connect a Provider" 에서 멈춘다). 다만 이벤트 이름·페이로드·전달 경로가 claude 와
동일함이 확인됐으므로, **추정: 프로바이더 연결 시 그대로 동작한다.**

## 10-2. 네 CLI 종합 (훅/이벤트 계층)

| CLI | 실측 수준 |
|---|---|
| claude code | ✅ 실제 턴에서 `UserPromptSubmit`→🟢, `Stop`→🔴 까지 브리지 통과 |
| codex cli | ✅ TUI 에서 `SessionStart`/`UserPromptSubmit`/`PreToolUse`/`Stop`/**`PermissionRequest`**→🟡 발화 (신뢰 게이트 넘긴 뒤) |
| opencode | ✅ SSE 이벤트 스펙 확인 + 어댑터·픽스처 테스트 (브리지 실구동은 미검증) |
| qwen code | ✅ `SessionStart` 발화 + 브리지 통과. 나머지 이벤트는 프로바이더 필요 |
