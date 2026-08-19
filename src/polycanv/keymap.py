"""누른 키 → PTY 로 보낼 바이트.

**Textual 의 `event.character` 만으로는 터미널이 안 된다.** 실측하면 `ctrl+c`, `enter`,
화살표가 모두 `None` 이다 — 즉 명령 하나 실행할 수 없다. `backspace` 는 `\\x08` 로
오는데 유닉스 터미널은 `\\x7f` 를 기대하고, `delete` 는 반대로 `\\x7f` 로 온다.

그래서 이름으로 직접 옮긴다. 표는 xterm 이 보내는 것을 따른다 — 안에서 도는 프로그램들이
그걸 기준으로 만들어져 있기 때문이다.
"""

from __future__ import annotations

#: 이름이 붙은 키들. 값은 xterm 이 보내는 시퀀스.
NAMED = {
    "enter": "\r",
    "tab": "\t",
    "shift+tab": "\x1b[Z",
    "escape": "\x1b",
    "backspace": "\x7f",  # 유닉스는 DEL 을 지우기로 쓴다. \x08 을 보내면 안 지워진다
    "delete": "\x1b[3~",
    "insert": "\x1b[2~",
    "up": "\x1b[A",
    "down": "\x1b[B",
    "right": "\x1b[C",
    "left": "\x1b[D",
    "home": "\x1b[H",
    "end": "\x1b[F",
    "pageup": "\x1b[5~",
    "pagedown": "\x1b[6~",
    "space": " ",
    # 제어 문자 중 글자로 안 떨어지는 것들
    "ctrl+space": "\x00",
    "ctrl+backslash": "\x1c",
    "ctrl+right_square_bracket": "\x1d",
    "ctrl+circumflex_accent": "\x1e",
    "ctrl+underscore": "\x1f",
}

#: 기능키. F1–F4 만 SS3 를 쓴다 — 역사적인 이유고, 프로그램들이 그렇게 읽는다.
FUNCTION = {
    "f1": "\x1bOP",
    "f2": "\x1bOQ",
    "f3": "\x1bOR",
    "f4": "\x1bOS",
    "f5": "\x1b[15~",
    "f6": "\x1b[17~",
    "f7": "\x1b[18~",
    "f8": "\x1b[19~",
    "f9": "\x1b[20~",
    "f10": "\x1b[21~",
    "f11": "\x1b[23~",
    "f12": "\x1b[24~",
}


def sequence(key: str, character: str | None) -> str | None:
    """이 키를 PTY 로 보낼 바이트로. 보낼 것이 없으면 `None`."""
    if key in NAMED:
        return NAMED[key]
    if key in FUNCTION:
        return FUNCTION[key]

    if key.startswith("ctrl+"):
        name = key[len("ctrl+") :]
        # ctrl+a → \x01 … ctrl+z → \x1a. 대문자로 눌러도 같은 바이트다.
        if len(name) == 1 and name.isalpha():
            return chr(ord(name.lower()) - 96)
        return None

    if key.startswith("alt+"):
        # alt 는 앞에 ESC 를 붙여 보낸다(meta prefix). readline 의 alt+b 같은 것들이 이걸 읽는다.
        name = key[len("alt+") :]
        # `alt+b` 는 `character` 가 비어 오므로(실측) 이름의 글자를 그대로 쓴다.
        inner = name if len(name) == 1 and name >= " " else sequence(name, character)
        return f"\x1b{inner}" if inner else None

    # 남은 것은 그냥 찍히는 글자다.
    if character is not None and character >= " ":
        return character
    return None
