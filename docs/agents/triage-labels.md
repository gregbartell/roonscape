# Ticket Statuses

The skills speak in terms of five canonical triage roles. This repo uses each
role's name as its local Markdown status string and adds one terminal delivery
state for completed implementation work.

| Status            | Kind     | Meaning                                  |
| ----------------- | -------- | ---------------------------------------- |
| `needs-triage`    | Triage   | Maintainer needs to evaluate this issue  |
| `needs-info`      | Triage   | Waiting on reporter for more information |
| `ready-for-agent` | Triage   | Fully specified, ready for an AFK agent  |
| `ready-for-human` | Triage   | Requires human implementation            |
| `wontfix`         | Triage   | Closed without implementation            |
| `done`            | Delivery | Implemented, verified, and accepted      |

When a skill mentions a canonical triage role (for example, "apply the
AFK-ready triage label"), use the corresponding triage status from this table.
`done` is not a triage recommendation: it replaces `ready-for-agent` or
`ready-for-human` only after that ticket's implementation and acceptance work
is complete.

Treat `done` as the authoritative completion signal. Acceptance checkboxes and
implementation comments provide supporting evidence, but agents should not
reconstruct completion from those clues when the status is already `done`.
`wontfix` remains distinct: it means the ticket closed without delivering its
requested behavior.
