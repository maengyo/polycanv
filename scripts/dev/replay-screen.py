"""캡처한 PTY 바이트를 터미널로 재생해 **실제 보이는 화면**을 뽑는다.

ANSI 시퀀스를 해석하지 않으면 커서 이동·지우기가 무시돼 화면이 뒤죽박죽이 된다.
그래서 터미널 에뮬레이터(pyte)에 그대로 먹인다.

pyte 는 일부 질의 시퀀스(DSR 등)에서 예외를 던진다 — 우리는 화면만 필요하므로 삼킨다.
"""

import contextlib
import sys

import pyte


class Tolerant(pyte.HistoryScreen):
    # 터미널이 "너 누구냐" 물어보는 시퀀스들. 응답할 상대가 없으니 무시한다.
    def report_device_status(self, *args, **kwargs):
        pass

    def report_device_attributes(self, *args, **kwargs):
        pass


rows, cols = int(sys.argv[2]), int(sys.argv[3])
screen = Tolerant(cols, rows)
stream = pyte.ByteStream(screen)
with open(sys.argv[1], "rb") as src:
    data = src.read()

# 한 바이트씩 먹여 예외가 나도 그 지점만 건너뛴다
chunk = 4096
for i in range(0, len(data), chunk):
    with contextlib.suppress(Exception):
        stream.feed(data[i : i + chunk])

for line in screen.display:
    print(line.rstrip())
