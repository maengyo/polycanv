"""polycanv 의 색.

**여태 색을 고른 적이 없었다.** Textual 기본값(`textual-dark`, primary `#0178D4`)이
그대로 나오고 있었고, 캔버스가 남색이던 것도 그래서다. 기본값을 쓰는 것은 선택이 아니다.

고른 기준 셋:

1. **캔버스는 물러나고 패널이 앞에 선다.** 바닥이 패널보다 튀면 눈이 빈 곳으로 간다.
2. **강조색은 파랑 계열로 묶는다.** 빨강·노랑·초록은 신호등이 쓸 자리다(🟢🟡🔴).
   지금 그 색을 테두리에 쓰면 나중에 신호등이 켜져도 구분이 안 된다.
3. **화면의 대부분은 우리가 안 그린다.** 안에서 도는 CLI 가 자기 색으로 그리므로,
   우리 색은 그 주위를 감싸는 틀이다. 틀이 시끄러우면 내용이 안 읽힌다.
"""

from __future__ import annotations

from textual.theme import Theme

#: 어두운 쪽. 오래 보는 화면이라 대비를 세게 주지 않는다.
DARK = Theme(
    name="polycanv-dark",
    dark=True,
    background="#0d0f14",  # 캔버스 — 가장 뒤
    surface="#1d212a",  # 패널 본문 — CLI 가 그리는 자리
    panel="#2a2f3a",  # 제목 줄 같은 덧대는 면
    foreground="#d3d7e0",
    primary="#7aa2f7",  # 포커스된 테두리
    secondary="#3d4455",  # 포커스 없는 테두리
    accent="#7aa2f7",
    warning="#e0af68",
    error="#f7768e",
    success="#9ece6a",
)

#: 밝은 쪽. 캔버스를 흰색으로 두지 않는다 — 그러면 흰 패널이 바닥에 묻힌다.
LIGHT = Theme(
    name="polycanv-light",
    dark=False,
    # ⚠️ 층 사이를 넉넉히 벌린다. 트루컬러가 아닌 터미널에서는 256색으로 떨어지는데,
    #    미세한 차이는 거기서 **같은 색으로 뭉개진다**(실측: #dcdee5 와 #fbfbfd 가
    #    둘 다 231 번으로 갔다).
    background="#c9ccd6",  # 캔버스 — 패널보다 뚜렷하게 어둡게
    surface="#ffffff",  # 패널 본문
    panel="#e6e8ee",
    foreground="#23262d",
    primary="#2f6bb0",
    secondary="#a8afbd",
    accent="#2f6bb0",
    warning="#a35c00",  # 밝은 바닥에서 읽히도록 어둡게 잡는다
    error="#c02c47",
    success="#2f7d32",
)

THEMES = (DARK, LIGHT)

#: 사용자가 말할 이름 → 실제 테마 이름.
BY_NAME = {"dark": DARK.name, "light": LIGHT.name}


def other(name: str) -> str:
    """지금 테마의 반대쪽."""
    return LIGHT.name if name == DARK.name else DARK.name
