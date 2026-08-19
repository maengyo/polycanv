#!/bin/sh
# polycanv 를 브라우저에서 연다.
#
#   sh scripts/polycanv-web.sh            서버 시작 + 토큰 발급 + 접속 주소 안내
#   sh scripts/polycanv-web.sh --stop     서버 중지
#   sh scripts/polycanv-web.sh --status   상태 확인
#
# 왜 있는가 — WSL·원격 머신처럼 **터미널 에뮬레이터를 직접 띄우기 번거로운 환경**에서
# 브라우저만으로 polycanv 를 쓰기 위해서다. 서버는 zellij 0.44+ 에 내장돼 있고
# polycanv 플러그인은 그 위에서 그대로 돈다 — 웹 전용 코드가 따로 없다.
#
# ⚠️ 서버는 기본으로 **127.0.0.1 에만** 바인딩한다. 외부에 열려면 `web_server_ip` 를
#    바꿔야 하는데, 그건 **터미널 접근 권한을 네트워크에 여는 일**이다.
#    HTTPS(`web_server_cert`/`web_server_key`)와 토큰 없이는 절대 하지 마라.

set -eu

PORT="${POLYCANV_WEB_PORT:-8082}"
ACTION="${1:-start}"

case "$ACTION" in
  --stop)
    zellij web --stop
    exit 0
    ;;
  --status)
    zellij web --status
    exit 0
    ;;
esac

command -v zellij >/dev/null 2>&1 || {
    printf '✗ zellij 가 없다. scripts/install.sh 를 먼저 보라.\n' >&2
    exit 1
}

# 0.44 미만에는 `web` 하위 명령이 없다
zellij web --help >/dev/null 2>&1 || {
    printf '✗ 이 zellij 에는 웹 서버가 없다 (0.44+ 필요). 현재: %s\n' \
        "$(zellij --version 2>/dev/null)" >&2
    exit 1
}

printf '\npolycanv — 브라우저로 열기\n\n'

if zellij web --status 2>/dev/null | grep -q online; then
    printf '  서버가 이미 떠 있다\n'
else
    zellij web --daemonize >/dev/null 2>&1
    printf '  서버 시작 (127.0.0.1:%s)\n' "$PORT"
fi

# 토큰은 발급 시 한 번만 보인다. 이미 있으면 새로 만들지 않는다 —
# 계속 만들면 목록만 지저분해지고 옛 토큰은 여전히 유효하다.
if zellij web --list-tokens 2>/dev/null | grep -q .; then
    printf '  기존 로그인 토큰을 쓴다 (목록: zellij web --list-tokens)\n'
    printf '  잊었으면: zellij web --revoke-all-tokens 후 이 스크립트를 다시 실행\n'
else
    printf '\n  로그인 토큰 (이번 한 번만 표시된다):\n'
    zellij web --create-token 2>&1 | sed 's/^/    /'
fi

printf '\n  접속: http://127.0.0.1:%s\n' "$PORT"
printf '\n  세션이 없으면 먼저 띄워라:\n'
printf '    zellij --config config/keybinds.kdl -s polycanv -n layouts/polycanv.kdl\n'
printf '\n  중지: sh scripts/polycanv-web.sh --stop\n\n'
