# Command line and MCP


```
tokenwhy report                     summary for the last 14 days, long lists capped
tokenwhy report --full              every row of every section
tokenwhy report --section startup   one section in full (live, findings, startup, skills, memory, subagents, compactions, tools)
tokenwhy findings --window 7d       ranked recommendations
tokenwhy tools --project myproj     tool output by command type
tokenwhy live                       today and the active sessions
tokenwhy report --anon --format md  report to share, names replaced
tokenwhy report --color always      force colour (default auto: terminal only, NO_COLOR respected; never for md or json)
tokenwhy --help                     every command and option
```

Findings carry a rule code (R1 to R18); the line `Rules:` under the list says what each code checks.

`--provider <id>` keeps one agent (`claude-code`, `codex`, `agy`, `opencode`). `--root <id>=<path>` reads another
folder for that agent; the app takes the same list from the environment:
`TOKENWHY_ROOT=claude-code=D:\fx\projects;codex=D:\fx\codex`. A bare path without `<id>=` is accepted only
when a single provider is compiled.

## ccusage import (optional)

`tokenwhy import ccusage <file>` stores the daily totals of a ccusage export made with
`ccusage daily --by-agent --json > ccusage.json` (tokenwhy never runs ccusage). In the app, turn it on under
Settings > ccusage import and give the file path there. Per agent and day, transcripts read by tokenwhy win and
the export fills only the days they lack; agents tokenwhy does not read (amp, ...) always count. Rows without an
agent are refused. Importing the same file again replaces its rows. Imported totals feed the daily figures, the
week and today only, labelled "imported (ccusage)"; never sessions, tools or findings.

## Claude Code stats-cache (estimated history)

Claude Code deletes transcripts after 30 days but keeps `stats-cache.json` in its config folder (`~/.claude`, or each
`CLAUDE_CONFIG_DIR`) with tokens per day and model going back months. tokenwhy reads it at each scan when the file
changed, offline, and counts its days only where it has no Claude transcript day of its own for that account. The file
gives one total per day and model: tokenwhy splits it by that model's all-time input, output, cache read and cache
write shares from the same file and prices cache writes at the 5-minute rate, so these days' cost is marked `est.`.
They feed the daily history only (project `(stats-cache)`), never sessions, tools or findings. The file is
undocumented; a shape other than version 5 is ignored with one message. Cargo feature `claude-stats`, on by default.

## Reports for agents

`tokenwhy report --for-agent` prints a short markdown brief to paste into an agent session: fixes ranked by
dollars a week saved and share of spend, each with its evidence and the exact instruction to apply. `--anon`,
`--window`, `--project` and `--provider` apply; `--format json` gives the contract in `docs/json-contract.md`.

## Use with your agent

Paste a brief into the session: `tokenwhy report --for-agent` prints the fixes to apply, ranked by dollars a week.

Or let the agent ask tokenwhy itself over MCP (local stdio, read-only, no network). In the app, Settings > Agents >
Copy MCP setup copies the line for the installed exe; with the CLI:

```
claude mcp add --scope user tokenwhy -- <path to>/tokenwhy mcp --anon
```

The app exe (`tokenwhy.exe mcp`) uses the default settings and ignores options; the CLI takes them.

Tools: `get_findings(window_days?, project?, provider?)`, `explain_spend(project? | session?)`,
`suggest_fixes(limit?)`, `get_summary()`. Options given after `mcp` (`--window`, `--project`, `--provider`,
`--anon`, `--root`) are the defaults of every call. Results follow `docs/json-contract.md`.
