#!/bin/sh
# polycanv 설치 — 플러그인을 빌드해 zellij 플러그인 디렉터리에 놓는다.
#
#   sh scripts/install.sh            빌드 + 설치
#   sh scripts/install.sh --check    설치 없이 환경만 점검
#
# 되풀이해서 실행해도 안전하다(멱등).

set -eu

PLUGIN_DIR="${POLYCANV_PLUGIN_DIR:-$HOME/.config/zellij/plugins}"
TARGET=wasm32-wasip1
MIN_ZELLIJ=0.43.0
REPO=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)

say()  { printf '  %s\n' "$1"; }
fail() { printf '\n✗ %s\n' "$1" >&2; exit 1; }

# ── 환경 점검 ────────────────────────────────────────────────────────────────
printf '\npolycanv 설치\n\n환경 점검\n'

command -v zellij >/dev/null 2>&1 || fail "zellij 가 없다. 설치: brew install zellij (또는 배포판 패키지)"
zellij_version=$(zellij --version 2>/dev/null | awk '{print $2}')
say "zellij       $zellij_version"

# 0.43.0 미만이면 replace_pane_with_existing_pane 이 없어 뷰 전환이 성립하지 않는다.
lowest=$(printf '%s\n%s\n' "$MIN_ZELLIJ" "$zellij_version" | sort -t. -k1,1n -k2,2n -k3,3n | head -1)
[ "$lowest" = "$MIN_ZELLIJ" ] || fail "zellij $MIN_ZELLIJ 이상이 필요하다 (현재 $zellij_version).
    뷰 전환이 쓰는 replace_pane_with_existing_pane 이 $MIN_ZELLIJ 에서 추가됐다."

command -v cargo >/dev/null 2>&1 || fail "cargo 가 없다. 설치: https://rustup.rs
    (Homebrew rustup 은 keg-only 다 — PATH 에 \$(brew --prefix rustup)/bin 을 넣어라)"
say "cargo        $(cargo --version 2>/dev/null | awk '{print $2}')"

if ! rustup target list --installed 2>/dev/null | grep -qx "$TARGET"; then
    fail "wasm 타깃이 없다. 설치: rustup target add $TARGET
    (주의: 오래된 문서의 wasm32-wasi 가 아니라 $TARGET 이다)"
fi
say "wasm 타깃    $TARGET"

# 빌드가 디스크를 꽤 먹는다. 가득 찬 상태에서는 링크 단계에서 엉뚱한 오류가 난다.
avail_kb=$(df -k "$REPO" | awk 'NR==2{print $4}')
if [ "$avail_kb" -lt 1048576 ]; then
    say "⚠ 디스크 여유가 $((avail_kb / 1024))MB 뿐이다. 빌드가 실패할 수 있다 (권장 1GB 이상)."
fi

if [ "${1:-}" = "--check" ]; then
    printf '\n✓ 점검 통과. 설치하려면 인자 없이 다시 실행해라.\n\n'
    exit 0
fi

# ── 빌드 ─────────────────────────────────────────────────────────────────────
printf '\n빌드 (release)\n'
cd "$REPO"
cargo build --release --target "$TARGET" -p polycanv-sidebar -p polycanv-launcher -p polycanv-status
say "완료"

# ── 설치 ─────────────────────────────────────────────────────────────────────
printf '\n설치 → %s\n' "$PLUGIN_DIR"
mkdir -p "$PLUGIN_DIR"
for wasm in polycanv-sidebar polycanv-launcher polycanv-status; do
    src="target/$TARGET/release/$wasm.wasm"
    [ -f "$src" ] || fail "$src 가 없다. 빌드가 조용히 실패했을 수 있다."
    cp "$src" "$PLUGIN_DIR/"
    say "$(printf '%-20s %s' "$wasm.wasm" "$(du -h "$PLUGIN_DIR/$wasm.wasm" | awk '{print $1}')")"
done

printf '\n✓ 설치 완료\n\n실행:\n'
printf '    zellij --config %s/config/keybinds.kdl -s polycanv -n %s/layouts/polycanv.kdl\n' "$REPO" "$REPO"
printf '\n최초 실행 시:\n'
printf '    사이드바 패인에 권한 요청이 뜬다. y 로 승인해라.\n'
printf '    승인 전에는 로드만 되고 키에 반응하지 않는다 — 고장이 아니다.\n'
printf '\n신호등을 켜려면 docs/setup.md 6장(훅 배선)을 보라.\n\n'
