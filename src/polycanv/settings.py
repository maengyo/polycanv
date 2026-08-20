"""남겨 두는 설정.

지금은 테마 하나뿐이지만, **고른 것이 다음에도 남아야** 고른 보람이 있다.

`tools.toml` 과 같은 규칙을 따른다: **어떤 경우에도 예외를 내지 않는다.**
설정 파일 하나 때문에 프로그램이 안 뜨면 사용자는 polycanv 가 고장 난 줄 안다.
"""

from __future__ import annotations

import contextlib
import sys
from dataclasses import dataclass
from pathlib import Path

from .theme import DARK, THEMES
from .tools import config_path

if sys.version_info >= (3, 11):
    import tomllib
else:
    import tomli as tomllib

VALID = {t.name for t in THEMES}


def settings_path() -> Path:
    return config_path().with_name("settings.toml")


@dataclass
class Settings:
    theme: str = DARK.name

    def save(self, path: Path | None = None) -> None:
        """조용히 실패한다 — 테마를 못 적었다고 작업을 막을 이유가 없다."""
        path = path or settings_path()
        with contextlib.suppress(OSError):
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(f'theme = "{self.theme}"\n', encoding="utf-8")


def load(path: Path | None = None) -> Settings:
    path = path or settings_path()
    try:
        data = tomllib.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, tomllib.TOMLDecodeError):
        return Settings()

    theme = data.get("theme")
    # 모르는 이름이면 기본값으로 돌아간다. 손으로 고쳤다가 오타를 냈을 수도 있다.
    return Settings(theme=theme if theme in VALID else DARK.name)
