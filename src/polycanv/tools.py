"""캔버스에 띄울 도구 목록.

**코드가 아니라 파일에 둔다.** 기본으로 들어 있는 여섯 개는 예시일 뿐 특권이 없다.
자기 CLI 를 한 줄 적어 넣으면 내장 도구와 똑같이 동작해야 한다.
"""

from __future__ import annotations

import os
import shutil
import sys
from dataclasses import dataclass
from pathlib import Path

if sys.version_info >= (3, 11):
    import tomllib
else:  # 3.10 에는 tomllib 이 없다
    import tomli as tomllib

#: 설정이 없을 때 만들어 주는 내용. **주석까지 함께 쓴다** — 빈 파일을 받은 사용자는
#: 무엇을 어떻게 적어야 하는지 알 수 없고, 대개 문서를 찾으러 가지 않는다.
DEFAULT_TOML = """\
# polycanv 가 띄울 수 있는 도구들.
#
# name    목록에 보일 이름. 마음대로 바꿔도 된다.
# command 실행할 명령. 첫 항목이 실행 파일이고, 이걸로 설치 여부를 판단한다.
# cwd     (선택) 시작 디렉터리. 없으면 polycanv 를 띄운 자리에서 시작한다.

[[tool]]
name = "shell"
command = ["$SHELL"]

[[tool]]
name = "claude"
command = ["claude"]

[[tool]]
name = "codex"
command = ["codex"]

[[tool]]
name = "opencode"
command = ["opencode"]

[[tool]]
name = "qwen"
command = ["qwen"]
"""


def _expand(value: str) -> str:
    return os.path.expanduser(os.path.expandvars(value))


@dataclass(frozen=True)
class Tool:
    """띄울 수 있는 것 하나."""

    name: str
    command: tuple[str, ...]
    cwd: str | None = None

    def resolved(self) -> list[str]:
        """`$SHELL`, `~` 를 펼친 실제 명령."""
        return [_expand(arg) for arg in self.command]

    def resolved_cwd(self) -> str | None:
        """펼친 시작 디렉터리.

        펼치지 않으면 `~/work/api` 가 글자 그대로 쓰여 `chdir` 이 실패하고, 그 실패는
        조용히 삼켜져 **엉뚱한 디렉터리에서 도구가 뜬다.** 문서에 적어 둔 표기다.
        """
        return _expand(self.cwd) if self.cwd else None

    @property
    def executable(self) -> str:
        return self.resolved()[0]

    def available(self) -> bool:
        """지금 이 기계에서 실행할 수 있는가.

        **이름이 아니라 실행 파일로 본다.** 항목 이름을 "내 클로드"로 바꿔도
        판정이 달라지면 안 된다.
        """
        exe = self.executable
        if os.path.sep in exe:
            return os.path.isfile(exe) and os.access(exe, os.X_OK)
        return shutil.which(exe) is not None


@dataclass(frozen=True)
class ToolConfig:
    """읽어 들인 도구 목록과, 읽으면서 생긴 문제."""

    tools: list[Tool]
    path: Path
    #: 설정을 못 읽었을 때의 사유. 이 경우 `tools` 는 기본값이다.
    problem: str | None = None


def config_path() -> Path:
    base = os.environ.get("XDG_CONFIG_HOME") or os.path.join(os.path.expanduser("~"), ".config")
    return Path(base) / "polycanv" / "tools.toml"


def _parse(raw: bytes) -> list[Tool]:
    data = tomllib.loads(raw.decode("utf-8"))
    entries = data.get("tool")
    # `[[tool]]` 이 아니라 `[tool]` 로 적으면 dict 가, `tool = "x"` 로 적으면 문자열이 온다.
    # 문법은 맞으니 해석은 통과하는데, 그대로 돌면 엉뚱한 것을 뒤져서 터진다.
    if not isinstance(entries, list):
        return []
    tools: list[Tool] = []
    for entry in entries:
        if not isinstance(entry, dict):
            continue
        command = entry.get("command")
        # 이름이나 명령이 빠진 항목은 조용히 건너뛴다. 하나가 잘못됐다고
        # 나머지 도구를 전부 못 쓰게 만들 이유가 없다.
        if not isinstance(command, list) or not command:
            continue
        name = entry.get("name") or str(command[0])
        tools.append(
            Tool(
                name=str(name),
                command=tuple(str(arg) for arg in command),
                cwd=str(entry["cwd"]) if entry.get("cwd") else None,
            )
        )
    return tools


def defaults() -> list[Tool]:
    return _parse(DEFAULT_TOML.encode("utf-8"))


def load(path: Path | None = None) -> ToolConfig:
    """설정을 읽는다. 없으면 만들어 준다.

    **어떤 경우에도 예외를 내지 않는다.** 설정 파일 하나 때문에 도구가 안 뜨면
    사용자는 polycanv 가 고장 난 줄 안다. 문제는 `problem` 으로 알린다.
    """
    path = path or config_path()
    try:
        raw = path.read_bytes()
    except FileNotFoundError:
        try:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(DEFAULT_TOML, encoding="utf-8")
        except OSError as exc:
            return ToolConfig(defaults(), path, f"설정을 만들지 못했습니다: {exc}")
        return ToolConfig(defaults(), path)
    except OSError as exc:
        return ToolConfig(defaults(), path, f"설정을 읽지 못했습니다: {exc}")

    try:
        tools = _parse(raw)
    except (tomllib.TOMLDecodeError, UnicodeDecodeError) as exc:
        return ToolConfig(defaults(), path, f"설정을 해석하지 못했습니다: {exc}")

    if not tools:
        return ToolConfig(defaults(), path, "쓸 수 있는 도구가 없어 기본값을 씁니다")
    return ToolConfig(tools, path)
