Hand-built transcripts for the tests of design section 11. Numbers are chosen so expected values are computed by
hand; each test writes the arithmetic in a comment.

Naming: `<provider>/<test_name>/` holding the transcript tree as the provider stores it, e.g.
`claude/workflow_agents/proj/<session>.jsonl` and `claude/workflow_agents/proj/<session>/subagents/agent-a.jsonl`.
A single-file case may be `<provider>/<test_name>.jsonl`. `<test_name>` is the test function name of section 11
(`stream_dedup`, `resume_copy`, `workflow_agents`, `incremental_equals_full`, `compaction`, `skill_carry`,
`classify`, `startup_parts`, `anon_leak`). Metrics tests that need no provider format build `FileRecords` in code.
