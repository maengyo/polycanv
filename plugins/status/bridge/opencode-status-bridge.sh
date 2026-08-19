#!/usr/bin/env bash
# opencode 의 SSE 이벤트를 상태 플러그인으로 흘려보내는 브리지.
#
# 왜 셸 스크립트인가 — wasm 플러그인 안에서는 SSE 를 읽을 수 없다. zellij 0.44.3 의
# `web_request` 는 단발성이고 `run_command` 는 프로세스가 끝나야 출력을 준다. 무한 스트림에
# 둘 다 맞지 않는다. 반면 `zellij pipe` 는 STDIN 스트리밍을 지원한다.
#
# 해석은 하지 않는다. 원문 한 줄을 그대로 넘기고, 상태 매핑은 Rust 어댑터가 한다
# (`plugins/status/src/adapters/opencode.rs`). 여기서 jq 로 파싱하기 시작하면 테스트가 사라진다.
#
# 사용:
#   opencode-status-bridge.sh --port 47311 --pane 3
#   opencode-status-bridge.sh --discover --pane 3     # lsof 로 포트를 찾는다
#
# 포트는 런처가 `opencode --port <N>` 으로 직접 지정하는 것이 가장 견고하다.
# TUI 는 포트를 랜덤 배정하고 파일로 노출하지 않는다 (근거: docs/research/cli-status-hooks.md §3-5).

set -uo pipefail

PORT=""
PANE="${ZELLIJ_PANE_ID:-}"
HOST="127.0.0.1"
DISCOVER=0
PLUGIN="${POLYCANV_STATUS_PLUGIN:-file:$HOME/.config/zellij/plugins/polycanv-status.wasm}"

while [ $# -gt 0 ]; do
  case "$1" in
    --port) PORT="$2"; shift 2 ;;
    --pane) PANE="$2"; shift 2 ;;
    --host) HOST="$2"; shift 2 ;;
    --plugin) PLUGIN="$2"; shift 2 ;;
    --discover) DISCOVER=1; shift ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

if [ -z "$PANE" ]; then
  echo "브리지는 어느 패인의 상태인지 알아야 한다: --pane <id> 또는 \$ZELLIJ_PANE_ID" >&2
  exit 2
fi

discover_port() {
  # opencode 가 LISTEN 중인 첫 포트. 여러 인스턴스가 뜨면 엉뚱한 것을 잡을 수 있으므로
  # --port 지정이 언제나 낫다.
  lsof -nP -iTCP -sTCP:LISTEN 2>/dev/null \
    | awk '/opencode/ { split($9, a, ":"); print a[length(a)]; exit }'
}

if [ "$DISCOVER" = "1" ] && [ -z "$PORT" ]; then
  PORT="$(discover_port)"
fi

if [ -z "$PORT" ]; then
  echo "포트를 알 수 없다: --port <n> 또는 --discover" >&2
  exit 2
fi

pipe_args=(--name polycanv:status --args "tool=opencode,pane_id=${PANE}")
# ★ --plugin 을 반드시 지정한다. 생략하면 실행 중인 **모든** 플러그인에게 브로드캐스트되는데,
#   그중 하나라도 ReadCliPipes 권한이 없어 unblock 하지 못하면 **파이프 전체가 매달린다**
#   (실측: 5분 초과. 오래된 인스턴스가 남아 있기만 해도 걸린다 — CLAUDE.md 참조).
#   "파이프 이름으로 거르니 안전하다"는 직관은 틀렸다. 거르는 것은 처리 여부일 뿐,
#   막힌 파이프는 그대로 막혀 있다.
#
#   지정하면 플러그인이 안 떠 있을 때 zellij 가 새 인스턴스를 띄우는데, 그 인스턴스는
#   권한 미승인 상태라 역시 매달린다. 그래서 **레이아웃에 마운트돼 승인된 인스턴스**를
#   가리켜야 한다. 기본값은 설치 스크립트가 놓는 경로다.
[ -n "$PLUGIN" ] && pipe_args+=(--plugin "$PLUGIN")

# opencode 가 아직 안 떴거나 연결이 끊길 수 있다. 끊기면 다시 붙는다 —
# 붙어 있지 않은 동안의 완료 신호는 영영 놓치기 때문이다.
while true; do
  # ⚠️ /global/event 다. /event 가 아니다 — opencode 1.14.48 실측으로 /event 와
  # /event?directory=<cwd> 는 server.connected 하나만 주고 세션 이벤트가 오지 않는다.
  curl -sN --max-time 0 "http://${HOST}:${PORT}/global/event" \
    | zellij pipe "${pipe_args[@]}"
  sleep 1
done
