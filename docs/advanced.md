# Advanced

## Privacy details

- Read-only on transcripts: `~/.claude/projects`, `~/.codex`, the Antigravity folders under `~/.gemini` and
  `~/.local/share/opencode` (SQLite databases are read from a copy).
- Writes only its own cache: `%LOCALAPPDATA%	okenwhyecords.bin` on Windows, the platform cache folder elsewhere.
- `--anon` replaces project, session, path and unknown command names with aliases before any output;
  `--anon=strict` also hides skill, agent type, MCP server and memory file names.

## Memory and performance

Measured on Windows 11, release build, 5 min on a live transcript root. The tray-only mode (no window) is the
light option: about 27 to 37 MB for the tokenwhy process and no WebView2 process. The compact window adds the six
WebView2 processes of Tauri, about 90 to 145 MB private on top of the app's 35 to 40 MB, so the compact view uses
over 100 MB in total. That is the known cost of a webview for this proof of concept; switch to tray-only in the
settings to avoid it.

Settings > Appearance > Performance picks how the windows run; a change applies after a restart (button in the
settings and in the tray menu).

- Eco: WebView2 without a GPU process, one renderer, Low memory target, opaque widget, no animation. The figures
  above were measured in this mode.
- Normal (default): GPU on, system glass behind the widget (Acrylic on Windows 11, Blur on Windows 10) and the
  Glass theme, animations. Not yet measured; expect the GPU process on top of the Eco figures.
- Glass theme: see-through acrylic behind the dashboard on Windows 10 1809+ and 11 (only while the window has
  focus, and only with Settings > Personalisation > Colours > Transparency effects on), vibrancy on macOS. With
  transparency off, on Linux (no portable blur API) or when the window loses focus, Glass shows a gradient
  backdrop instead. The compact widget never takes focus, so it always shows that backdrop.

## Adding a theme

1. Write `app/web/src/styles/themes/<id>.css`. Scope every rule to `:root[data-theme="<id>"]`; set only the tokens
   that differ from the base ones in `app/web/src/styles/theme.css`, plus `color-scheme`, and add its `@import` to
   `app/web/src/styles/app.css`.
2. Add one line to `app/web/src/lib/themes.ts`:
   `{ id: '<id>', family: '<family>', label: 'Name', scheme: 'light' | 'dark', sw: ['<plane>', '<accent>'] }`.
   Add `needs: 'normal'` if the theme cannot run in Eco mode; it then shows its `scheme` theme there.

The settings list and the saved setting follow from that line; Rust stores the id as
plain text. A saved id that no longer exists shows the System theme.

## Layout

- `crates/core`: all analysis. A `Provider` trait turns one transcript format into neutral records;
  `providers/claude` is the Claude Code implementation. `pipeline` runs discover, cache, parse, metrics,
  rules and the snapshot for every interface.
- `crates/cli` and `app`: thin interfaces over `core::pipeline`, no analysis of their own.

## Build

A stable Rust toolchain; Node.js for the app's UI (Svelte 5 + Vite in `app/web`).

```
cargo build --release -p tokenwhy          # command line
npm ci --prefix app/web && npm run build --prefix app/web   # UI into app/ui-dist, needed before the app build
cargo build --release -p tokenwhy-app      # tray app (Tauri 2, needs WebView2 on Windows)
```

`cargo tauri build` (the installer) runs the UI build itself. For live UI work, run
`cargo tauri dev --config tauri.dev.conf.json` from `app/`.
