# JSON contract

Version 1 (`schema: 1`). Covers `tokenwhy report --for-agent --format json` and the results of the MCP tools
(`structuredContent` of each `tools/call`). The full report (`tokenwhy report --format json`) is the snapshot,
`schema: 2`, described by `app/web/snapshot.schema.json`. Its optional `weeks` is the same active-week count, the divisor of every per-week figure. Its optional `history_newer` (integer) is set when
`history.bin` was written by a newer tokenwhy: this build leaves the file untouched and shows only transcripts still
on disk. Its optional `plan` (builds with the `plan` feature) lists, per Claude account, the share of the
subscription limits used as of Claude Code's last `/usage` reading, cached in its `.claude.json`:
`{account, label, five_hour_pct, five_hour_resets_ms, seven_day_pct, seven_day_resets_ms, fetched_ms}`, percents
0 to 100, any of them `null` when the reading lacks it. The cache is undocumented; a shape tokenwhy does not know
leaves the list empty.

Daily history rows taken from Claude Code's `stats-cache.json` (feature `claude-stats`) carry `cost_usd.est: true`:
their tokens are split by each model's all-time shares and their cost is an estimate. Their project is
`(stats-cache)`, with 0 sessions.

Rules for readers such as the Stream Deck plugin:

- `schema` is bumped on any breaking change (field removed, renamed or retyped). New fields may appear in the same
  version; ignore the ones you do not know.
- Money is US dollars, rounded to cents. Times are Unix milliseconds. Shares are 0 to 1.
- A number that cannot be priced is `null`, never 0.
- With `--anon`, project, session and path names are aliases; `anonymised` is then `true`.

## Brief (`report --for-agent --format json`, MCP `suggest_fixes`)

| Field | Type | Meaning |
| - | - | - |
| `schema` | integer | 1 |
| `window` | `{from_ms, to_ms}` | analysed period |
| `weeks` | number | active weeks: 7-day blocks, counted back from the window end, with any use (calendar length under 7 days or with no use) |
| `spend_usd` | number or null | spend in the window (daily history, else sessions) |
| `spend_usd_per_week` | number or null | `spend_usd / weeks` |
| `saved_usd_per_week` | number | sum over every group, listed or not |
| `anonymised` | bool | names replaced |
| `partial` | bool | scan not finished |
| `items` | array of item | ranked, costliest first |
| `more` | integer | groups left out by the limit (8 by default) |

Item: one rule for one project and provider.

| Field | Type | Meaning |
| - | - | - |
| `rank` | integer | 1 for the first |
| `rule` | string | `R1` to `R18` |
| `what` | string | what the rule checks |
| `project`, `provider` | string or null | null when the finding spans all |
| `findings` | integer | findings merged into this group |
| `usd_saved_per_week` | number or null | estimate |
| `share_of_spend` | number or null | `usd_saved_per_week / spend_usd_per_week` |
| `tokens_saved_per_week` | number or null | estimate |
| `evidence` | array of string | lines of the costliest findings, at most two |
| `fix` | string | instruction to apply, from `crates/core/src/agent.rs` |

## MCP tools

All tools are read-only. Each result has `content` (one text block: the JSON below, or the markdown brief for
`suggest_fixes`), `structuredContent` and `isError`. A failed scan returns `isError: true` with the message.

- `get_findings(window_days?, project?, provider?)`: `{schema, window, anonymised, spend_usd, spend_usd_per_week,
  findings}`; each finding is the snapshot's finding (`provider`, `rule`, `project`, `line`,
  `tokens_saved_per_week`, `usd_saved_per_week`, `co2_g_saved_per_week`, `usd_saved_last_7d`, each amount
  `{value, est}`) plus `fix`.
- `explain_spend(project? | session?)`: with `session` (id or prefix): `{schema, window, anonymised, session,
  fixes}` where `session` is a snapshot session row and `fixes` up to three brief items of its project, or
  `error` when the session is not in the window. Otherwise: `{schema, window, anonymised, spend_usd,
  spend_usd_per_week, projects: [{project, cost_usd, sessions}], top_sessions, fixes}`.
- `suggest_fixes(limit?)`: the brief above.
- `get_summary()`: `{schema, window, anonymised, partial, spend_usd, spend_usd_per_week, today_usd, sessions,
  active_sessions, saved_usd_per_week, top_fixes: [{rule, project, usd_saved_per_week, fix}]}`.

## Protocol

MCP revision 2026-07-28 (https://modelcontextprotocol.io/specification/2026-07-28), dual-era: a request with
`_meta["io.modelcontextprotocol/protocolVersion"] = "2026-07-28"` is served without a handshake
(`server/discover`, `tools/list`, `tools/call`, `ping`); an `initialize` request selects the legacy handshake
(2025-11-25, 2025-06-18 or 2025-03-26) for the rest of the process. Transport: stdio, one JSON-RPC 2.0 message
per line.
