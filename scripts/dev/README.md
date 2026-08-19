# 개발용 도구

제품에 들어가지 않는다. **화면을 눈으로 확인하기 위한 것들**이다.

TUI 는 돌려 봐야 아는데, 에이전트는 화면을 못 본다. 그래서 PTY 를 직접 붙인다 —
**마스터가 받는 바이트가 곧 사용자가 보는 화면 전체**이므로, 그걸 적어 두고 재생하면
무엇이 그려졌는지 확인할 수 있다.

실제로 이 방식으로 잡은 것들:

- `ctrl+n` 이 도구 목록을 안 열고 셸로 흘러갔다 → 접두키 도입
- `enter` 와 `ctrl+c` 가 PTY 로 전혀 전달되지 않았다 → `keymap.py`
- 패널을 캔버스 밖으로 끌어낼 수 있었다 → 이동·크기에 경계 추가

## 1. `capture-screen.py` + `replay-screen.py` — 화면 확인

```sh
OUT=/tmp/shot.raw ROWS=30 COLS=104 SECONDS=8 KEYS=$'\x02n' \
  uv run python scripts/dev/capture-screen.py polycanv

uv run python scripts/dev/replay-screen.py /tmp/shot.raw 30 104
```

`KEYS` 는 `WARMUP` 초 뒤부터 `GAP` 초 간격으로 한 글자씩 넣는다.

**주의 둘.**

- 창 크기는 자식을 실행하기 **전에** 잡아야 한다. 나중에 바꾸면 이미 읽은 뒤라 반영되지
  않는다(실측).
- 키를 **붙여 쓰면 안 된다.** `\x02n` 을 한 번에 써 넣었더니 접두키가 먹지 않고 `n` 이
  셸에 찍혔다. 앱이 한 덩어리로 읽어 처리 순서가 달라진다. 한 글자씩 띄워 보내야 한다.

## 2. `record-demo.py` — README 에 넣을 GIF

```sh
SCRIPT=docs/demo/launcher.txt SECONDS=22 \
  uv run --with pillow python scripts/dev/record-demo.py docs/demo/launcher.gif
```

스크립트로 조작을 적는다. 마우스도 넣을 수 있어 **끌어서 옮기고 크기 바꾸는 장면**을
만들 수 있다.

```
wait 1.5
key ctrl+b
key n
type echo hello
drag 20,3 68,3      # 눌러서 끌어다 놓기 (칸 좌표, 0 부터)
```

**녹화 환경을 반드시 씻어야 한다.** 그냥 찍으면 프롬프트에 사용자명·호스트명·실제 경로가
그대로 들어가고, 그게 공개 저장소의 이미지로 남는다. `HOME`, `SHELL`, `XDG_CONFIG_HOME`
을 임시 디렉터리로 돌리고 프롬프트를 고정해서 찍는다.

폰트는 두 벌을 섞는다. 박스 문자는 Menlo 가, 한글은 Apple SD Gothic Neo 가 갖고 있고 둘
다 가진 폰트가 없다. 한 벌로 그리면 어느 쪽이든 두부가 된다.

## 무엇을 여전히 확인할 수 없나

**색과 움직임이 눈에 어떻게 보이는지.** 위 도구들은 문자와 색 값을 보여줄 뿐이다.
실제 느낌은 사람이 봐야 한다.
