"""도구 설정 읽기.

**어떤 입력에도 예외를 내지 않아야 한다.** 설정 파일 하나 때문에 도구가 안 뜨면
사용자는 polycanv 자체가 고장 난 줄 안다.
"""

from __future__ import annotations

import os
from pathlib import Path

from polycanv import tools


def test_설정이_없으면_만들어_준다(tmp_path: Path) -> None:
    path = tmp_path / "sub" / "tools.toml"
    config = tools.load(path)

    assert path.exists(), "첫 실행에서 설정 파일이 생겨야 편집할 대상이 생긴다"
    assert config.problem is None
    assert [t.name for t in config.tools][0] == "shell", "빠른 길이 첫 항목이어야 한다"


def test_기본_설정은_다시_읽어도_같다(tmp_path: Path) -> None:
    path = tmp_path / "tools.toml"
    tools.load(path)
    again = tools.load(path)

    assert [t.name for t in again.tools] == [t.name for t in tools.defaults()]


def test_망가진_설정은_기본값으로_버틴다(tmp_path: Path) -> None:
    path = tmp_path / "tools.toml"
    path.write_text("[[tool]\nname = ", encoding="utf-8")

    config = tools.load(path)

    assert config.tools, "해석에 실패해도 도구 목록은 비면 안 된다"
    assert config.problem is not None, "조용히 넘어가면 사용자는 편집이 먹은 줄 안다"


def test_항목_하나가_잘못돼도_나머지는_쓴다(tmp_path: Path) -> None:
    path = tmp_path / "tools.toml"
    path.write_text(
        '[[tool]]\nname = "broken"\n\n[[tool]]\nname = "good"\ncommand = ["echo"]\n',
        encoding="utf-8",
    )

    config = tools.load(path)

    assert [t.name for t in config.tools] == ["good"]


def test_이름을_바꿔도_설치_판정은_그대로다() -> None:
    """판정은 라벨이 아니라 실행 파일로 한다."""
    renamed = tools.Tool(name="내 클로드", command=("sh",))
    assert renamed.available()

    assert not tools.Tool(name="sh", command=("polycanv-does-not-exist",)).available()


def test_환경변수와_물결표를_펼친다(monkeypatch) -> None:
    monkeypatch.setenv("SHELL", "/bin/sh")
    assert tools.Tool(name="shell", command=("$SHELL",)).resolved() == ["/bin/sh"]

    home = os.path.expanduser("~")
    assert tools.Tool(name="x", command=("~/bin/x",)).resolved() == [f"{home}/bin/x"]


def test_절대경로는_실행권한까지_본다(tmp_path: Path) -> None:
    script = tmp_path / "thing"
    script.write_text("#!/bin/sh\n", encoding="utf-8")

    assert not tools.Tool(name="thing", command=(str(script),)).available()
    script.chmod(0o755)
    assert tools.Tool(name="thing", command=(str(script),)).available()


def test_표를_배열로_안_적어도_버틴다(tmp_path: Path) -> None:
    """`[[tool]]` 대신 `[tool]` 로 적는 실수는 흔하고, TOML 문법으로는 통과한다."""
    for text in ('[tool]\nname = "x"\ncommand = ["sh"]\n', 'tool = "sh"\n', "tool = 3\n"):
        path = tmp_path / "tools.toml"
        path.write_text(text, encoding="utf-8")

        config = tools.load(path)

        assert config.tools, f"기본값으로라도 떠야 한다: {text!r}"
        assert config.problem is not None


def test_시작_디렉터리의_물결표를_펼친다() -> None:
    """펼치지 않으면 chdir 이 조용히 실패해 엉뚱한 곳에서 도구가 뜬다."""
    tool = tools.Tool(name="api", command=("sh",), cwd="~/work/api")

    assert tool.resolved_cwd() == f"{os.path.expanduser('~')}/work/api"
    assert tools.Tool(name="x", command=("sh",)).resolved_cwd() is None


def test_설정에_적은_시작_디렉터리를_읽는다(tmp_path: Path) -> None:
    path = tmp_path / "tools.toml"
    path.write_text(
        '[[tool]]\nname = "api"\ncommand = ["sh"]\ncwd = "~/work/api"\n', encoding="utf-8"
    )

    (tool,) = tools.load(path).tools

    assert tool.resolved_cwd() == f"{os.path.expanduser('~')}/work/api"
