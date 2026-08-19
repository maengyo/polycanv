#!/bin/sh
# polycanv 상태 훅 브리지 — CLI 훅 JSON(stdin) → 사이드바 신호등
#
#   사용: CLI 훅 설정에서 이 스크립트를 command 훅으로 지정한다.
#   claude:  ~/.claude/settings.json 의 hooks
#   codex:   $CODEX_HOME/config.toml 의 [[hooks.<event>]]
#   qwen:    ~/.qwen/settings.json (claude 호환)
#
# 설계 메모:
#  - **파싱하지 않고 흘려보내지 않는다.** 여기서 이벤트 이름만 읽어 상태로 매핑하고,
#    나머지 판단은 하지 않는다. 복잡한 판별은 어댑터(Rust)의 몫이다.
#  - 자기 패인은 zellij 가 주입하는 ZELLIJ_PANE_ID 로 안다(실측 확인).
#  - 실패해도 **절대 0 이외로 끝내지 않는다.** 훅이 실패하면 CLI 가 멈추거나 경고를 낸다.
#    신호등이 안 켜지는 것보다 사용자의 CLI 를 방해하는 쪽이 훨씬 나쁘다.

set -u
SIDEBAR="${POLYCANV_SIDEBAR_PLUGIN:-file:$HOME/.config/zellij/plugins/polycanv-sidebar.wasm}"

# zellij 밖에서 실행됐으면 조용히 나간다 (CLI 를 단독으로 쓰는 경우)
[ -n "${ZELLIJ_PANE_ID:-}" ] || exit 0

payload=$(cat 2>/dev/null || true)

# hook_event_name 은 PascalCase 로 온다 (codex/claude 공통 — 실측)
event=$(printf '%s' "$payload" | tr -d ' \n' | sed -n 's/.*"hook_event_name":"\([A-Za-z]*\)".*/\1/p')
[ -n "$event" ] || exit 0

case "$event" in
    UserPromptSubmit|PreToolUse|PostToolUse) state=running ;;
    Notification|PermissionRequest)          state=waiting ;;
    Stop|SubagentStop)                       state=finished ;;
    SessionStart|SessionEnd)                 state=idle ;;
    *)                                       exit 0 ;;   # 모르는 이벤트는 무시한다
esac

# 밀리초. date +%s%3N 은 BSD date 에 없으므로 초 * 1000 으로 만든다.
at_ms=$(( $(date +%s) * 1000 ))

event_json=$(printf '{"pane":{"terminal":%s},"state":"%s","source":"hook","at_ms":%s}' \
    "$ZELLIJ_PANE_ID" "$state" "$at_ms")

# ★ 페이로드는 **인자**로 넘긴다. STDIN 으로 주면 `zellij pipe` 가 스트리밍 모드로 들어간다.
#
# ★★ 그리고 **절대 기다리지 않는다.** `zellij pipe` 가 반환하지 않는 경우가 실측으로 확인됐다
#    (페이로드를 실은 CLI 파이프가 수 분간 매달림 — 플러그인 인스턴스가 새로 뜨면서 권한
#    승인 대기에 걸리는 것으로 추정되나 원인은 완전히 좁히지 못했다).
#    훅이 안 끝나면 **CLI 의 턴이 통째로 멈춘다.** 신호등 하나 때문에 사용자의 작업을 세우는
#    것은 어떤 이유로도 정당화되지 않는다. 그래서:
#      - 백그라운드로 던지고 즉시 빠진다
#      - 감시 프로세스가 정해진 시간 뒤 반드시 죽인다 (매달린 프로세스가 쌓이지 않게)
POLYCANV_PIPE_TIMEOUT="${POLYCANV_PIPE_TIMEOUT:-5}"
(
    zellij pipe --name "polycanv:state" --plugin "$SIDEBAR" -- "$event_json" >/dev/null 2>&1 &
    pipe_pid=$!
    ( sleep "$POLYCANV_PIPE_TIMEOUT"; kill "$pipe_pid" 2>/dev/null ) >/dev/null 2>&1 &
    wait "$pipe_pid" 2>/dev/null
) >/dev/null 2>&1 &

exit 0
