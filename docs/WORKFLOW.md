# Workflow — how this build is run

This document describes the operating loop used to build `cc-livive-net`. It is
process documentation, not product documentation. `concept.md` remains the
authoritative technical brief; where this file and `concept.md` disagree about
the product, `concept.md` wins.

## Roles

| Role | Who | Responsibility |
|---|---|---|
| Owner | Jarad | Product direction, answers to escalated questions, final say |
| Project manager / integrator | Lead agent | Task briefs, sub-agent dispatch, review, verification, git, docs |
| Implementer | Sub-agent | Reads its brief, asks questions, implements, reports evidence |

Sub-agents are **not** committers and do not run git. All commits are made by
the project manager after independent verification.

## The loop

1. **Brief.** The PM writes `docs/tasks/M<N>-<slice>.md` describing scope,
   file ownership, invariants, test gates, and out-of-scope work.
2. **Dispatch.** The PM starts a sub-agent with a minimal prompt: repo path,
   brief path, "read the brief and the relevant code", and "return open
   questions before writing code".
3. **Questions.** The implementer returns questions only. No files are touched.
4. **Resolution.** The PM answers anything that only requires applying
   `concept.md` or an existing decision. Anything that changes product
   behaviour, public API surface, budget, or scope is escalated to the Owner
   with a recommendation.
5. **Implement.** The PM resumes the same implementer session with the answers.
6. **Report.** The implementer returns a summary of changes plus pasted
   command output as evidence.
7. **Verify.** The PM re-runs the gates in Docker, reads the diff, and fixes or
   sends back anything that does not meet the brief.
8. **Commit.** The PM commits to `main` in small, green steps and updates
   `docs/STATUS.md` and `docs/DECISIONS.md`.

## Ground rules

- **Docker only.** No native `cargo`, `rustc`, `wasm-pack`, or `node` on the
  host for build/test purposes. Use `./scripts/docker.sh`.
- **Anything green, always green.** `main` must pass `./scripts/docker.sh check`
  at every commit. A red commit gets fixed forward immediately.
- **No silent convention changes.** Facelet order, cubie order, orientation
  definitions, and move order are fixed by `concept.md` §3. Changing one is a
  breaking event requiring an ADR and a re-baselined golden hash.
- **Tests are the contract.** A milestone is done when its gates pass, not when
  the code "looks right".
- **One writer per tree.** Parallel implementers are only allowed in separate
  git worktrees with disjoint file ownership.
- **No vendored solver code.** Algorithms are reimplemented from published
  descriptions; GPL sources are never copied.

## Escalation policy (autopilot)

The PM may decide without asking when the choice is small, reversible, and
consistent with `concept.md`. The PM must escalate when a decision:

- changes the public API in `concept.md`, or the wire format of the WASM module;
- changes a **DECIDED** convention;
- changes scope (adds/removes a crate, milestone, or platform);
- risks budget (browser payload, table size, build time) beyond the stated
  limits;
- cannot be made to work as specified (blocker).

## Commit conventions

Prefixes: `feat`, `fix`, `test`, `docs`, `build`, `ci`, `refactor`, `chore`.
Scope is the crate or area (`core`, `solver`, `cli`, `wasm`, `web`, `docker`,
`pm`). Examples:

```
build(docker): add reproducible rust+wasm dev image
feat(core): implement cube state, facelets, and 18 moves
test(core): property tests for facelet/cubie round-trip
docs(pm): record M0 decisions
```

Commit bodies state *why*, and name the gates that were run.
