"""캔버스 위에서 터미널이 실제로 도는지, 자리와 크기를 지키는지."""

from polycanv.app import PolycanvApp
from polycanv.terminal import MIN_HEIGHT, MIN_WIDTH


async def test_터미널이_뜨고_출력이_들어온다():
    app = PolycanvApp()
    async with app.run_test(size=(100, 30)) as pilot:
        panel = app.canvas.open_terminal(
            ["/bin/sh", "-c", "echo POLYCANV_HELLO; sleep 30"], title="t"
        )
        await pilot.pause(1.2)
        body = "".join(panel.vt.display)
        assert "POLYCANV_HELLO" in body, f"PTY 출력이 화면에 안 들어왔다: {body[:80]!r}"
        panel.close()


async def test_새_터미널은_겹치지_않게_놓인다():
    # 정확히 겹치면 뒤엣것이 안 보여서 사용자는 열리지 않은 줄 안다.
    app = PolycanvApp()
    async with app.run_test(size=(120, 40)) as pilot:
        a = app.canvas.open_terminal(["/bin/sh", "-c", "sleep 30"], title="a")
        b = app.canvas.open_terminal(["/bin/sh", "-c", "sleep 30"], title="b")
        await pilot.pause(0.4)
        assert (a.geometry_.x, a.geometry_.y) != (b.geometry_.x, b.geometry_.y)
        a.close()
        b.close()


async def test_크기를_바꾸면_PTY_도_따라간다():
    # 이걸 빼먹으면 안에서 도는 프로그램이 화면 크기를 잘못 알고 깨진다.
    app = PolycanvApp()
    async with app.run_test(size=(120, 40)) as pilot:
        panel = app.canvas.open_terminal(["/bin/sh", "-c", "sleep 30"], title="t")
        await pilot.pause(0.4)
        panel.resize_to(60, 20)
        cols, rows = panel.geometry_.inner()
        assert (panel.vt.columns, panel.vt.lines) == (cols, rows)
        panel.close()


async def test_최소_크기_아래로는_줄지_않는다():
    # 너무 작아지면 안에서 도는 프로그램이 화면을 못 그린다.
    app = PolycanvApp()
    async with app.run_test(size=(120, 40)) as pilot:
        panel = app.canvas.open_terminal(["/bin/sh", "-c", "sleep 30"], title="t")
        await pilot.pause(0.4)
        panel.resize_to(1, 1)
        assert panel.geometry_.width == MIN_WIDTH
        assert panel.geometry_.height == MIN_HEIGHT
        panel.close()


async def test_옮기면_화면상_위치가_따라간다():
    app = PolycanvApp()
    async with app.run_test(size=(120, 40)) as pilot:
        panel = app.canvas.open_terminal(["/bin/sh", "-c", "sleep 30"], title="t")
        await pilot.pause(0.4)
        panel.move_to(30, 12)
        await pilot.pause(0.3)
        assert (panel.region.x, panel.region.y) == (30, 12), "styles.offset 이 화면에 반영돼야 한다"
        panel.close()


async def test_음수_좌표로는_나가지_않는다():
    # 왼쪽/위로 끌어내면 화면 밖으로 사라져 되찾을 수 없다.
    app = PolycanvApp()
    async with app.run_test(size=(120, 40)) as pilot:
        panel = app.canvas.open_terminal(["/bin/sh", "-c", "sleep 30"], title="t")
        await pilot.pause(0.4)
        panel.move_to(-20, -8)
        assert (panel.geometry_.x, panel.geometry_.y) == (0, 0)
        panel.close()


async def test_크기를_줄여도_화면_내용이_남는다():
    """크기 조절은 이 도구의 핵심 조작이다. 그때마다 내용이 날아가면 못 쓴다.

    `pyte.Screen.resize` 는 줄이 줄어들 때 **위쪽을 버린다** — 커서가 1행에 있어도
    12줄→7줄로 줄이면 내용이 통째로 사라진다(실측). 그래서 원본 바이트를 재생한다.
    """
    app = PolycanvApp()
    async with app.run_test(size=(120, 40)) as pilot:
        panel = app.canvas.open_terminal(
            ["/bin/sh", "-c", "echo KEEP_ME_AFTER_RESIZE; sleep 30"], title="t"
        )
        await pilot.pause(1.2)
        assert "KEEP_ME_AFTER_RESIZE" in "".join(panel.vt.display)

        panel.resize_to(46, 9)  # 줄이기
        await pilot.pause(0.3)
        assert "KEEP_ME_AFTER_RESIZE" in "".join(panel.vt.display), "줄였더니 내용이 사라졌다"

        panel.resize_to(80, 24)  # 다시 키우기
        await pilot.pause(0.3)
        assert "KEEP_ME_AFTER_RESIZE" in "".join(panel.vt.display), "키웠더니 내용이 사라졌다"
        panel.close()


async def test_재생_버퍼가_무한정_자라지_않는다():
    # 오래 띄워둔 세션이 메모리를 먹으면 안 된다.
    from polycanv.terminal import REPLAY_LIMIT

    app = PolycanvApp()
    async with app.run_test(size=(120, 40)) as pilot:
        panel = app.canvas.open_terminal(["/bin/sh", "-c", "sleep 30"], title="t")
        await pilot.pause(0.4)
        panel._remember(b"x" * (REPLAY_LIMIT * 2))
        assert len(panel._replay) == REPLAY_LIMIT
        panel.close()
