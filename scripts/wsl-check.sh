#!/usr/bin/env bash
# WSL 에서 polycanv 를 쓸 수 있는지 확인한다.
#
# **회사 기계에서 한 번 돌리고 출력을 그대로 가져오면 된다.** 아무것도 설치하지 않고,
# 아무것도 바꾸지 않는다. 읽기만 한다.
#
#   bash wsl-check.sh
set -u

ok()   { printf '  \033[32m✓\033[0m %s\n' "$1"; }
no()   { printf '  \033[31m✗\033[0m %s\n' "$1"; }
warn() { printf '  \033[33m!\033[0m %s\n' "$1"; }
head_() { printf '\n\033[1m%s\033[0m\n' "$1"; }

head_ "1. 여기가 어디인가"
if grep -qi microsoft /proc/version 2>/dev/null; then
  ok "WSL 이다: $(grep -oi 'microsoft.*' /proc/version | head -1)"
  # WSL2 는 별도 네트워크 네임스페이스라 localhost 전달 방식이 다르다
  if [ -e /run/WSL ] || grep -qi 'wsl2' /proc/version 2>/dev/null; then
    ok "WSL2 로 보인다 (Windows 가 localhost 를 넘겨준다)"
  else
    warn "WSL1 일 수 있다 — 네트워크를 Windows 와 공유한다"
  fi
else
  warn "WSL 이 아니다 (그래도 나머지 확인은 유효하다)"
fi
echo "  배포판: $(. /etc/os-release 2>/dev/null && echo "$PRETTY_NAME" || echo 알 수 없음)"

head_ "2. 파이썬"
if command -v python3 >/dev/null; then
  v=$(python3 -c 'import sys;print("%d.%d"%sys.version_info[:2])')
  if python3 -c 'import sys;exit(0 if sys.version_info>=(3,10) else 1)'; then
    ok "python3 $v"
  else
    no "python3 $v — 3.10 이상이 필요하다"
  fi
  python3 -c 'import pty,fcntl,termios,struct' 2>/dev/null \
    && ok "pty 를 쓸 수 있다 (터미널을 띄우는 데 이게 전부다)" \
    || no "pty 를 못 쓴다 — 이게 안 되면 polycanv 가 동작하지 않는다"
else
  no "python3 이 없다"
fi

head_ "3. 설치 수단"
command -v uv >/dev/null && ok "uv $(uv --version 2>/dev/null | awk '{print $2}')" || warn "uv 가 없다 (오프라인 꾸러미로도 설치할 수 있다)"
command -v pip3 >/dev/null && ok "pip3 있다" || warn "pip3 이 없다"

head_ "4. 바깥으로 나갈 수 있는가  ← 여기가 핵심이다"
probe() {  # 이름, URL
  if command -v curl >/dev/null; then
    code=$(curl -s -o /dev/null -w '%{http_code}' -m 8 "$2" 2>/dev/null)
    [ "$code" = "200" ] && ok "$1 에 닿는다" || no "$1 에 못 닿는다 (HTTP ${code:-없음})"
  else
    warn "curl 이 없어 $1 을 확인 못 했다"
  fi
}
probe "PyPI" "https://pypi.org/simple/"
probe "PyPI 파일 서버" "https://files.pythonhosted.org/"
[ -n "${PIP_INDEX_URL:-}" ] && ok "사내 미러가 설정돼 있다: $PIP_INDEX_URL" || warn "사내 미러(PIP_INDEX_URL) 설정 없음"
[ -f ~/.pip/pip.conf ] && ok "~/.pip/pip.conf 있다: $(grep -i index ~/.pip/pip.conf 2>/dev/null | head -1)"
[ -f ~/.config/pip/pip.conf ] && ok "~/.config/pip/pip.conf 있다: $(grep -i index ~/.config/pip/pip.conf 2>/dev/null | head -1)"

head_ "5. 브라우저로 열 수 있는가"
python3 - <<'PY' 2>/dev/null || echo "  (파이썬이 없어 건너뜀)"
import http.server, socket, threading, urllib.request
srv = http.server.HTTPServer(("127.0.0.1", 0), http.server.SimpleHTTPRequestHandler)
port = srv.server_address[1]
threading.Thread(target=srv.handle_request, daemon=True).start()
try:
    urllib.request.urlopen(f"http://127.0.0.1:{port}/", timeout=3)
    print(f"  \033[32m✓\033[0m WSL 안에서 127.0.0.1:{port} 가 열린다")
except Exception as e:
    print(f"  \033[31m✗\033[0m 로컬 포트를 못 연다: {e}")
srv.server_close()
PY
echo "  ※ Windows 브라우저에서 열리는지는 사람이 확인해야 한다:"
echo "     WSL 에서  python3 -m http.server 8000"
echo "     Windows 브라우저에서  http://localhost:8000  이 열리면 된다"

head_ "6. 있으면 좋은 것"
command -v tmux >/dev/null && ok "tmux $(tmux -V 2>/dev/null | awk '{print $2}') — 붙었다 떼기에 쓸 수 있다" || warn "tmux 없음"
command -v claude >/dev/null && ok "claude 있다" || warn "claude 없음"
command -v codex  >/dev/null && ok "codex 있다"  || warn "codex 없음"
[ -n "${WSL_DISTRO_NAME:-}" ] && echo "  배포판 이름: $WSL_DISTRO_NAME"
command -v clip.exe >/dev/null && ok "clip.exe 있다 (WSL→Windows 복사가 된다)" || warn "clip.exe 없음"

head_ "정리"
echo "  이 출력을 그대로 가져오면 됩니다."
