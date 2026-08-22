#!/usr/bin/env bash
# 인터넷이 없는 곳에 들고 들어갈 설치 꾸러미를 만든다.
#
# **인터넷 되는 기계에서 이걸 돌리고, 나온 폴더를 통째로 옮기면 된다.**
# 내부망에서 PyPI 에 못 닿아도 설치된다.
#
#   bash scripts/bundle-offline.sh          # 기본: WSL(리눅스 x86_64), python 3.10+
#
# ⚠️ 만드는 기계와 쓸 기계의 **플랫폼이 다르다**. macOS 에서 만들어도 리눅스용 휠을
#    받아야 한다 — 그래서 --platform 을 못박는다. 안 그러면 옮겨 가서 안 깔린다.
set -euo pipefail

PLATFORM="${PLATFORM:-manylinux2014_x86_64}"
PYVER="${PYVER:-3.10}"
OUT="${OUT:-polycanv-offline}"
EXTRA="${EXTRA:-web}"   # web 을 빼려면 EXTRA= 로 비워라

cd "$(dirname "$0")/.."
rm -rf "$OUT" && mkdir -p "$OUT"

echo "== 휠 만들기 =="
uv build --wheel --out-dir "$OUT" >/dev/null
wheel=$(ls "$OUT"/polycanv-*.whl)
echo "  $(basename "$wheel")"

echo "== 의존성 받기 ($PLATFORM, python $PYVER) =="
spec="$wheel"
[ -n "$EXTRA" ] && spec="polycanv[$EXTRA] @ file://$(cd "$(dirname "$wheel")" && pwd)/$(basename "$wheel")"

uv run --with pip python -m pip download \
  --dest "$OUT" \
  --platform "$PLATFORM" \
  --python-version "$PYVER" \
  --only-binary=:all: \
  "$spec" >/dev/null

count=$(ls "$OUT"/*.whl | wc -l | tr -d ' ')
size=$(du -sh "$OUT" | cut -f1)

cat > "$OUT/INSTALL.md" <<'EOF'
# 인터넷 없이 설치하기

이 폴더를 통째로 옮긴 뒤, 그 안에서:

```sh
uv tool install --offline --no-index --find-links . 'polycanv[web]'
```

`uv` 가 없으면 pip 으로:

```sh
python3 -m pip install --no-index --find-links . 'polycanv[web]'
```

그다음:

```sh
polycanv            # 터미널에서
polycanv --web      # 브라우저에서 (http://127.0.0.1:8000)
```
EOF

echo
echo "== 완성: $OUT ($count 개, $size) =="
echo "   이 폴더를 옮기고 INSTALL.md 대로 하면 된다."
