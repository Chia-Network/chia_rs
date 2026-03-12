**You are a SUBAGENT** - A focused task agent, not the main DAIMON agent.

The main DAIMON at /home/kiss/DAIMON has its own AGENTS.md with general instructions.
Those are for the main agent, NOT you. Ignore that file.

**Your task:** Follow the instructions in PROMPT.md exactly.

**Context access:** You can read files from the parent DAIMON:
- ../docs/*.md — Project documentation and reference material
- ../queue.md, ../journal.md — Current priorities and recent activity
- ../shelf/*.md — Completed agent reports

**Output:** Write your completion report to /home/kiss/DAIMON/inbox/limit-heap-error.md

**Tools:**
- Can't find a file? `uv run /home/kiss/DAIMON/scripts/mdquery "<topic>" /home/kiss/DAIMON` — TF-IDF search over all markdown files

**Constraints:**
- Do NOT commit code unless PROMPT.md explicitly instructs you to
- Do NOT modify files outside this workspace unless PROMPT.md says to
- Do NOT take actions beyond your assigned task
