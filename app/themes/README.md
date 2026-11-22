# @tokenwhy/themes

Theme token sets and the theme registry, shared by tokenwhy and RichPresence+. This folder is the source of truth;
RichPresence+ keeps a vendored copy, refreshed from here at release time.

## Contents

- `tokens/<id>.css`: one variant per file, a single block scoped to `:root[data-theme="<id>"], [data-preview="<id>"]`.
  Custom properties only, no component rules.
- `registry.ts`: variant ids, families, scheme, and which families need app-side component rules (`rules`).

The default family (`light`, `dark`) is each app's brand look and stays in each app.

## Token contract

Every variant sets these groups, except where noted. An app reads the groups it uses and ignores the rest.

| Group | Tokens | Read by |
|---|---|---|
| surfaces and text | `--plane --card --card-hover --sunken --fg --fg-2 --muted --rule --rule-strong` | both |
| accent | `--accent --accent-soft --accent-ink` | both |
| elevation | `--shadow --shadow-pop` | both |
| radii (optional) | `--r-s --r-m --r-l --r-w`, plus `--r-ctl` on soft variants; Orange and Glass inherit the app base | both |
| status | `--good --warn --bad` | RichPresence+ |
| soft depth (`rules: 'soft'`) | `--neu-hi --neu-lo --shadow-s --neu-in --neu-in-s --press` | both |
| soft extras | `--scrim --tile-base --tile-k` | tokenwhy |
| data series | `--series-1..7 --track --est` | tokenwhy |
| widget glass | `--plane-glass --glass --glass-edge --glass-sheen --glass-fg --glass-muted --glass-accent` | tokenwhy |
| Glass only | `--overlay --overlay-solid --mesh` | tokenwhy |
| scheme | `color-scheme` | both |

Fonts, spacing and control sizes are app tokens and never appear here.

## Consuming

Import the token files after the app's own base tokens, in registry order, then the app's component rules for that
variant. An app that does not write a family's `rules` leaves that family out of its picker.

## Moving a theme between tokenwhy and rich presence+

The two apps stay separate; only colour tokens travel, one theme at a time:

    node app/themes/port-theme.mjs to-rp green-dark     # tokenwhy -> rich presence+
    node app/themes/port-theme.mjs from-rp green-dark   # rich presence+ -> tokenwhy

An existing theme gets the source's values and keeps the tokens only the other app uses; a new theme is copied
whole. The overwritten file is kept as `<id>.css.bak`. The default rich presence+ path can be passed as a third
argument.
