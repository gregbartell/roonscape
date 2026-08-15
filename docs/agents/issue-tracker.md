# Issue tracker: Local Markdown

Issues and specs for this repo live as markdown files in `.scratch/`.

## Conventions

- One feature per directory: `.scratch/<feature-slug>/`
- The spec is `.scratch/<feature-slug>/spec.md`
- Implementation issues are one file per ticket at `.scratch/<feature-slug>/issues/<NN>-<slug>.md`, numbered from `01` — never a single combined tickets file
- Ticket state is recorded as a `Status:` line near the top of each issue file
  (see `triage-labels.md` for the triage roles and terminal `done` state)
- Comments and conversation history append to the bottom of the file under a `## Comments` heading

## When a skill says "publish to the issue tracker"

Create a new file under `.scratch/<feature-slug>/` (creating the directory if needed).

## When a skill says "fetch the relevant ticket"

Read the file at the referenced path. The user will normally pass the path or the issue number directly.

## Implementation ticket lifecycle

- `ready-for-agent` and `ready-for-human` mean the ticket is open, fully
  specified, and owned by the named kind of implementer. A `Blocked by:` edge
  can still prevent that implementer from starting immediately.
- A blocker is complete only when the referenced implementation ticket has
  `Status: done`. Do not infer completion from checked boxes, code that appears
  to exist, or an old implementation comment.
- After implementation, required checks, review, and acceptance all succeed,
  check the satisfied acceptance criteria and replace the ticket's status with
  `done` in the completing change.
- `done` is terminal for that ticket. Record newly discovered or intentionally
  deferred work in a follow-up ticket instead of leaving hidden work under a
  completed status.
- Exclude `done` and `wontfix` tickets when discovering work to implement.
  Their bodies and comments remain historical context.
- Specs retain their own overall status. Do not infer a spec's completion from
  one child ticket; evaluate its implementation tickets instead.

## Wayfinding operations

Used by `/wayfinder`. The **map** is a file with one **child** file per ticket.

- **Map**: `.scratch/<effort>/map.md` — the Notes / Decisions-so-far / Fog body.
- **Child ticket**: `.scratch/<effort>/issues/NN-<slug>.md`, numbered from `01`, with the question in the body. A `Type:` line records the ticket type (`research`/`prototype`/`grilling`/`task`); a `Status:` line records `claimed`/`resolved`.
- **Blocking**: a `Blocked by: NN, NN` line near the top. A ticket is unblocked when every file it lists is `resolved`.
- **Frontier**: scan `.scratch/<effort>/issues/` for files that are open, unblocked, and unclaimed; first by number wins.
- **Claim**: set `Status: claimed` and save before any work.
- **Resolve**: append the answer under an `## Answer` heading, set `Status: resolved`, then append a context pointer (gist + link) to the map's Decisions-so-far in `map.md`.
