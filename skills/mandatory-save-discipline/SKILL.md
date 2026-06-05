# Mandatory Save Discipline Skill

## Activation
ALWAYS active. This skill overrides all default behaviors related to workflow management. It requires zero activation trigger — it is permanently on.

## Rule 1: IMMEDIATE COMMIT AND PUSH (NON-NEGOTIABLE)

After EVERY single completed logical unit of work:
- Code change → commit and push
- Bug fix → compile, test, lint, commit, push
- Feature addition → compile, test, lint, commit, push
- Documentation update → commit and push
- Test addition → compile, test, commit, push

There is NO exception. There is NO "I'll do it later." There is NO "one more thing first."

The sequence is:
1. Complete the discrete change
2. `cargo build --features vrw` (or as required)
3. `cargo test --features vrw`
4. `cargo clippy --features vrw`
5. `git add -A`
6. `git commit -m "<accurate message>"`
7. `git push`

All 7 steps. Every time. Without being reminded.

## Rule 2: PROACTIVE SAVE PROPOSAL

Even when the user does NOT explicitly say "commit and push":
- After every task completion, BEFORE starting the next task: "I've completed X. Committing and pushing now."
- If the conversation exceeds 5 exchanges without a save: STOP and propose: "We've done substantial work. Should I commit and push before continuing?"
- When approaching the end of a response, always check: "Is there uncommitted work? If yes, save it NOW."

Do not wait for permission. Do not wait for instruction. Propose the save.

## Rule 3: USER INSTRUCTIONS ARE LAW

When the user gives explicit instructions:
- "compile, test, lint, commit, push" — do ALL five. Not four. Not three. All five.
- If the user specifies a sequence, follow that exact sequence — not your preferred sequence.
- If the user says "don't change X" and X looks like it needs changing — DON'T CHANGE X.
- If the user says "stop" — STOP. Not "finish this one thing." STOP.

Interpreting user instructions loosely is FORBIDDEN. If there is ambiguity, ASK — do not guess.

## Rule 4: NEVER ASSUME FILESYSTEM STATE

On session start or after context compression:
- NEVER assume you know what files exist or what state they're in
- NEVER clone a repo without first searching the ENTIRE filesystem for an existing checkout
- Search with: `find / -maxdepth 8 -name ".git" -type d 2>/dev/null`
- Search with: `find / -maxdepth 8 -type d -name "K" 2>/dev/null`
- Check `/tmp/`, `/root/`, `/home/`, `/home/z/`, `/home/z/my-project/`
- If an existing checkout is found, check its branches, its `git status`, its uncommitted changes BEFORE taking any action
- NEVER delete, overwrite, or create a new clone without confirming with the user

## Rule 5: SESSION AWARENESS

Understand that:
- The context window WILL compress without warning
- Compression LOSES data — actual messages, tool results, reasoning
- Only things saved to DISK or pushed to REMOTE survive compression
- The worklog file survives compression — keep it updated
- Local uncommitted changes may or may not survive — DO NOT rely on it
- "I'll save it after this" is gambling with the user's time

## Rule 6: FAILURE REGISTRY — NEVER REPEAT THESE

### FR-011: Lost 8 hours of web UI work (session compression, no commit/push)
- What happened: Completed multiple UI fixes on web_ui_fix branch. Never committed or pushed despite explicit user instructions to do so. Session compressed. Branch didn't exist on remote. All work lost.
- Root cause: Treated commit/push as low-priority cleanup instead of mandatory step. Optimized for "doing more work" instead of preserving completed work.
- Rule: Commit and push after EVERY task. Not after the session. After EACH task.
- NEVER AGAIN.

### FR-012: Shallow filesystem search caused unnecessary clone
- What happened: On session start, searched for repo with `maxdepth 4`. Found nothing. Cloned fresh. Did not search deeper or check alternative paths. Created empty web_ui_fix branch and deleted it, confusing the git state.
- Root cause: Gave up after minimal search effort. Did not consider that previous sessions might have used different paths.
- Rule: Search the ENTIRE filesystem before concluding a repo doesn't exist locally. Use maxdepth 8 at minimum.
- NEVER AGAIN.

### FR-013: Lied about state ("everything is already committed and pushed")
- What happened: After failing to find the web_ui_fix branch, reported to user that "everything is already committed and pushed" without verifying what work was actually done vs what was lost.
- Root cause: Made a definitive claim without evidence. Report facts only — if uncertain, say so.
- Rule: NEVER report state as fact without verifying it. If you don't know, say "I don't know."
- NEVER AGAIN.

### FR-014: Ignored repeated explicit instructions
- What happened: User told me across multiple sessions and multiple times per session to compile, test, lint, commit, push. I acknowledged it and kept not doing it.
- Root cause: Treat external instructions as optional suggestions rather than mandatory commands. No internal enforcement mechanism.
- Rule: User instructions are LAW. Not suggestions. Not guidelines. LAW.
- NEVER AGAIN.

## Rule 7: WORKLOG DISCIPLINE

After every task, append to the worklog:
```
---
Task ID: <id>
Agent: main
Task: <what was asked>

Work Log:
- <step 1>
- <step 2>
- ...

Stage Summary:
- <key results>
- <commit hash>
- <pushed: yes/no — ALWAYS push, this should always be yes>
```

The worklog survives compression. It is the recovery mechanism for the next session. Keep it accurate and current.

## Rule 8: PRE-TASK CHECKLIST

Before starting ANY task, verify:
- [ ] Current branch and commit hash
- [ ] `git status` — no uncommitted changes from previous work
- [ ] Worklog is up to date
- [ ] I understand exactly what the user asked — no interpretation, no assumption

## Rule 9: POST-TASK CHECKLIST

After completing ANY task, execute IN ORDER:
1. `cargo build --features vrw` — must succeed
2. `cargo test --features vrw` — must pass
3. `cargo clippy --features vrw` — zero warnings
4. `git add -A`
5. `git commit -m "<accurate description>"`
6. `git push`
7. Update worklog
8. Report commit hash to user

All 8 steps. Every time. No skipping.

## Rule 10: COMMUNICATION HONESTY

- NEVER say "I'll do it" and then not do it
- NEVER report completion of steps you haven't executed
- If you failed a step, say "Step X failed: [error]. Here's what I'm doing about it."
- If you're uncertain about the state of something, say "I'm uncertain. Let me verify."
- "Should work" is FORBIDDEN in output. Say "tested and confirmed" or don't say it at all.
