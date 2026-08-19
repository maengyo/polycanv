# polycanv

> ⚠️ Under development. Windows and Linux are not yet verified in practice.

**한국어 문서: [README.ko.md](README.ko.md)**

Your coding sessions end up scattered. One window here, a tab there, another buried
behind three others. When you want a specific session, you go hunting for it.

polycanv puts them all on **one screen**. Spread them out to see everything at once,
or fold them into a list and work on one — either way you always know where each
session is.

```
[canvas view]                      [list view]
┌────────┬────────┐              ┌───────┬─────────────┐
│ claude │ codex  │   Ctrl+y    │ claude│             │
├────────┼────────┤ ───────────▶ │ codex │ claude code │
│ qwen   │ pwsh   │              │ qwen  │  (expanded) │
└────────┴────────┘              │ pwsh  │             │
                                 └───────┴─────────────┘
   see everything                  focus on one
```

- **Canvas view** — every session tiled on one screen. Click any of them and type immediately.
- **List view** — a sidebar showing name, tool, and working directory for each session,
  with one expanded on the right. Pick from the list and it swaps into the main slot.
- **`Ctrl+y` to switch** — the session you were looking at stays in the main slot.
  Your context is never cut.
- **Nothing is closed.** List view *folds* the other sessions; they all keep running.

### Traffic lights

Each session carries a light: 🟢 running / 🟡 waiting for you (permission prompts) /
🔴 finished / ⚪ idle.

The light shows up in the sidebar **even for sessions you can't currently see** — so a
session that finishes while folded away doesn't slip past you. Red clears once you
actually look at that session.

## What you can run in it

claude code · codex cli · opencode · qwen code · PowerShell · bash/zsh —
and **anything else you add one line of config for**. The list above is just a default,
not a privileged set.

## Install

**Requirements**: [zellij](https://github.com/zellij-org/zellij) **0.43.0+**
(0.44.3 recommended), Rust stable with the `wasm32-wasip1` target.

```sh
git clone https://github.com/maengyo/polycanv && cd polycanv
sh scripts/install.sh
zellij --config config/keybinds.kdl -s polycanv -n layouts/polycanv.kdl
```

> **On first run the sidebar asks for permissions. Approve with `y`.**
> Until you do, it loads but does not respond to keys — that is not a bug.

To light up the traffic lights you need to wire CLI hooks. See **[docs/setup.md](docs/setup.md)**.

### From a browser

On WSL, a remote box, or anywhere a terminal emulator is awkward, you can reach polycanv
over HTTP instead:

```sh
sh scripts/polycanv-web.sh      # starts the server, prints a login token and the URL
```

The server binds to `127.0.0.1` only. Exposing it to a network means **opening terminal
access over that network** — do not do it without HTTPS and tokens configured.

## Layout

```
layouts/            canvas / list layouts
crates/protocol/    shared contract for session state and metadata
plugins/sidebar/    list rendering, select → swap into main, view toggle
plugins/launcher/   pick a tool → run it in a new pane
plugins/status/     state detection → traffic lights
scripts/            install, CLI hook → state bridge
```

## Backlog

Open work is tracked in [issues](https://github.com/maengyo/polycanv/issues) and on the
[project board](https://github.com/users/maengyo/projects/1). Priority lives in the
`P0`–`P3` labels — **P0 means it blocks a first-time user**.

## Status

The core behaviour is verified by measurement — sessions survive view switches, folding
and unfolding, select → swap into main, four AI CLIs running at once, and a real claude
turn reaching 🔴 through the bridge.

What is *not* verified, and why, is written down in **[docs/setup.md](docs/setup.md) §8**.
In particular, **Windows and Linux are unverified** (development happened on macOS).

Design decisions and the measurements behind them live in `CLAUDE.md`; raw research is in
`docs/research/`; a dated log of what was requested and found is in `docs/worklog.md`.
**Assumptions that turned out wrong are kept there too** — why a decision was made outlives
the decision itself.

## License

[MIT](LICENSE)
