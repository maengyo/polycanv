# polycanv

> ⚠️ Early development. macOS is where it is developed and tested; Linux should work but is
> unverified; Windows does not work yet (see below).

**한국어 문서: [README.ko.md](README.ko.md)**

Your coding sessions end up scattered. One window here, a tab there, another buried
behind three others. When you want a specific session, you go hunting for it.

polycanv puts them on **one canvas** — each terminal in a place you chose, at a size you
chose, staying where you put it.

![polycanv in action](docs/demo/launcher.gif)

*Opening a tool from the picker, dragging it into place, and resizing it.*

## How it works

- **A canvas, not a tiling grid.** Each terminal has a title bar: drag it to move the
  window, or use the minimize and close buttons at its right edge. Drag the bottom-right
  corner to resize. Terminals overlap, and they stay where you left them.
- **The body belongs to the terminal.** Drag across it to select text (`Cmd+C` copies);
  `Ctrl+C` goes to the program inside, where it interrupts a running agent. The wheel
  scrolls back through output — or is handed to the program inside when it wants the
  mouse, as claude code and vim do.
- **`Ctrl+b` then a key** — `n` opens the tool picker, `t` opens a shell, `w` closes the
  focused terminal, `q` quits. Everything else goes straight to the program inside, so
  `Ctrl+c`, `Ctrl+w` and the arrow keys still mean what they always meant. Press
  `Ctrl+b` twice to send it through.
- **Your tools, from a config file.** `~/.config/polycanv/tools.toml` is created on first
  run with claude code, codex, opencode, qwen and a shell. Add your own CLI with one
  entry — it behaves exactly like the built-ins.

## Install

One command. No terminal multiplexer to install first, no toolchain.

```sh
uv tool install polycanv
polycanv
```

Requires Python 3.10+, which `uv` will fetch for you if you do not have it.

> **Unix only for now.** Terminals are driven through `pty`, which does not exist on
> Windows. WSL works.

### From a browser

For WSL, a remote box, or anywhere a terminal emulator is awkward:

```sh
uv tool install --force 'polycanv[web]'
polycanv --web            # http://127.0.0.1:8000
polycanv --web --port 9000
```

Port 8000 is a busy address on most machines. If it is taken, polycanv moves to the
next free one and tells you where it went — but if you asked for a specific port and
it is taken, it says so rather than quietly opening somewhere else.

**It binds to `127.0.0.1` and there is no option to change that.** Serving polycanv over
HTTP is serving a shell, and there is no authentication yet. To reach it from another
machine, forward the port over SSH — `ssh -L 8000:127.0.0.1:8000 you@box` — which keeps
the authentication where it belongs.

### Seeing more at once

Terminals cannot scale text, so polycanv has no zoom of its own — but your terminal
already does it. `Cmd+-` (or `Ctrl+-`) shrinks the font, which gives the app more rows
and columns, which fits more panels on screen. In a browser the same shortcut works, and
you can start small with `?fontsize=8` on the URL.

## Configuring tools

```toml
[[tool]]
name = "claude"
command = ["claude"]

[[tool]]
name = "api server"
command = ["bash", "-lc", "npm run dev"]
cwd = "~/work/api"
```

A tool that is not installed still appears in the picker, marked — so you can tell
"polycanv cannot find it" apart from "polycanv does not support it".

## Planned

Tracked in [issues](https://github.com/maengyo/polycanv/issues) and on the
[project board](https://github.com/users/maengyo/projects/1):

- **Traffic lights** — 🟢 running / 🟡 waiting for you / 🔴 finished / ⚪ idle on each
  terminal's border. All four CLIs' state protocols are already measured and written up in
  [`docs/research/cli-status-hooks.md`](docs/research/cli-status-hooks.md); the detection
  itself is not built yet.
- **Grouping by project** — terminals working in the same directory read as one workstream.
- **Session persistence** — reopen to the arrangement you left.
- **Browser access** — for WSL and remote boxes, bound to `127.0.0.1` only.

## Prior art

The concept comes from **[cate](https://github.com/0-AI-UG/cate)** — *an infinite canvas
IDE for parallel coding agents*. cate makes the canvas a mission control: every terminal
shows whether its agent is working, finished, or waiting on you, and each git worktree gets
its own coloured territory, so five agents on five branches read as five separate
workstreams rather than a pile of tabs.

polycanv takes that idea to the terminal. Same premise — **sessions need a place, not a
tab index** — with a deliberately narrower scope: no editor, no browser, no desktop app.
If you want the full spatial IDE, use cate; it is the richer tool.

## Layout

```
src/polycanv/
  app.py        the app: prefix key, actions
  canvas.py     where terminals are placed
  terminal.py   one terminal: PTY, screen state, move and resize
  launcher.py   the tool picker
  tools.py      reading tools.toml
  keymap.py     key → the bytes a terminal expects
  keys.py       which keys the app takes, and why so few
scripts/dev/    recording the demo, capturing screens for verification
docs/           research, dated worklog, demo script
```

## Status

What is built: the canvas, terminals that move and resize, the tool picker, and key
handling that leaves the inner program its keys.

What is not: everything under **Planned** above. Design decisions and the measurements
behind them live in `CLAUDE.md`; a dated log is in `docs/worklog.md`. **Assumptions that
turned out wrong are kept there too** — why a decision was made outlives the decision.

## License

[MIT](LICENSE)
