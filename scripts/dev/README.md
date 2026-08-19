# 개발용 도구

제품에 들어가지 않는다. **plugins 화면을 눈으로 확인할 수 없다는 제약**을 우회하기 위한 것들이다.

## 왜 필요한가

zellij 는 플러그인 패인의 내용을 외부에 노출하지 않는다 — `zellij action dump-screen` 이
내장 플러그인(`tab-bar`, `status-bar`, `strider`)까지 포함해 전부 빈 출력이다(실측).
그래서 사이드바가 무엇을 그리는지 확인할 방법이 없다.

두 가지 우회로가 있고, 목적이 다르다.

## 1. `capture-screen.py` + `replay-screen.py` — 화면 전체

PTY 를 직접 붙여 zellij 를 띄우면 **마스터가 받는 바이트가 곧 사용자가 보는 화면 전체**다.
사이드바를 포함해 전부 들어 있다.

```sh
SESSION=demo OUT=/tmp/shot.raw ROWS=36 COLS=160 SECONDS=110 \
  python3 scripts/dev/capture-screen.py \
  zellij --config config/keybinds.kdl --session demo \
         --new-session-with-layout layouts/polycanv.kdl &

# 캡처가 도는 동안 세션을 원하는 상태로 만든다
zellij --session demo action new-pane --cwd ~/work/api -- claude
zellij --session demo action new-pane --cwd ~/work/web -- opencode

# 캡처가 끝나면 재생한다 (ANSI 를 해석해야 실제 화면이 나온다)
python3 scripts/dev/replay-screen.py /tmp/shot.raw 36 160
```

**주의**: 창 크기는 자식을 실행하기 **전에** 잡아야 한다. 나중에 바꾸면 zellij 가 이미 읽은
뒤라 반영되지 않는다(실측: VP_COLS 가 80 에 고정). 사이드바는 전체 폭의 22% 라 좁으면
내용이 잘려 검증이 무의미해진다.

## 2. `debug_render` — 사이드바가 그린 프레임만

사이드바 플러그인에 설정을 주면 그린 내용을 `[frame:NN]` 형태로 **zellij 로그**에 남긴다.
플러그인의 `eprintln!` 은 로그로 나가기 때문이다.

```kdl
plugin location="file:~/.config/zellij/plugins/polycanv-sidebar.wasm" {
    debug_render "true"
}
```

그 다음 로그에서 `[frame` 을 grep 한다. 로그 위치는 `$TMPDIR/zellij-<uid>/zellij-log/zellij.log`.

화면 전체가 필요 없고 **줄 단위 내용·하이라이트 위치·신호등 문자**만 확인할 때 이쪽이 빠르다.

## 무엇을 여전히 확인할 수 없나

**색과 깜빡임이 눈에 어떻게 보이는지.** 위 둘은 문자와 이스케이프 시퀀스를 보여줄 뿐이다.
실제 느낌은 사람이 터미널에서 봐야 한다.
