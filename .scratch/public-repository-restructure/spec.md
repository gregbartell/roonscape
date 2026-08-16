# Restructure RoonScape for public use

Status: done

## Problem Statement

RoonScape's source is already public, but the repository still reads like the
working directory for one private installation rather than an owner-ready
open-source project. Public documentation refers to a personal Reference
Deployment, an Intel NUC, a privileged 4K viewport, local Markdown tickets,
and a discarded prototype. The code and tests retain the prototype-derived
Gallery naming and deployment-derived sample identities. The Roon extension
publishes a personal email address, and the repository has no project license.

The repository's implementation is also spread across several top-level
directories even though its source has three clear modules and one shared
contract seam: the Roon bridge, the native renderer, the launcher, and the
language-neutral schemas and Fixture Scenarios they share. GitHub Issues are
disabled, while agent instructions still designate `.scratch` as the issue
tracker. This makes the project harder for a prospective owner, maintainer, or
agent to understand and gives obsolete personal context more authority than
the current product.

There are no releases or users, so compatibility with the present source and
archive layouts is not required. Observable product behavior must nevertheless
remain stable throughout what is primarily a repository and terminology
restructure.

## Solution

Present RoonScape as an owner-ready MIT-licensed project whose current tree is
free of personal-deployment assumptions. Group product source under one source
root while preserving the Roon bridge, native renderer, launcher, and shared
contract as distinct modules. Keep public documentation and maintainer scripts
easy to find at the repository root. Remove the obsolete prototype from the
current tree, replace its Gallery-derived implementation vocabulary with Now
Playing and presentation vocabulary, and use neutral Roon identities in
fixtures and tests.

Describe and validate a fluid presentation for common landscape displays
rather than treating one 4K deployment as canonical. Preserve the existing
runtime, packaging, release, and dependency behavior except where source-path
changes require mechanical updates.

Prepare the project to use GitHub Issues with guided bug and feature forms and
the canonical triage roles. Keep the current local specs and tickets in place
while this effort is implemented: these are intentionally the project's last
local tickets. Only the repository owner will delete `.scratch` after the
local effort is complete.

## User Stories

1. As a prospective RoonScape owner, I want the repository to describe a
   generally usable product, so that I can evaluate it without interpreting
   the maintainer's private installation history.
2. As a prospective RoonScape owner, I want personal host details absent from
   current documentation, so that I do not mistake one machine for a product
   requirement.
3. As a prospective RoonScape owner, I want RoonScape described in terms of a
   RoonScape Host, so that I understand that my compatible machine can run it.
4. As a listener, I want RoonScape to remain an unattended, read-only
   presentation, so that repository cleanup does not expand it into Roon
   Control.
5. As a listener, I want the presentation to remain truthful to the Tracked
   Output, Tracked Zone, and current Now Playing state, so that structural
   changes do not alter what RoonScape shows.
6. As an owner, I want the presentation to adapt to common landscape display
   resolutions, so that I do not need the maintainer's former 4K setup.
7. As an owner using a 1280x720 display, I want the presentation to remain
   composed and readable, so that the minimum supported landscape size is
   genuinely useful.
8. As an owner using a 4:3 display, I want the composition to use the available
   space without letterboxing, so that older or less common displays remain
   viable.
9. As an owner using a 16:9 display, I want fluid layout at ordinary and high
   resolutions, so that resolution changes do not change the product's visual
   character.
10. As an owner using a 16:10 display, I want the composition to preserve its
    hierarchy, so that a taller landscape display does not feel like a special
    case.
11. As an owner using an ultrawide display, I want RoonScape to remain
    intentional rather than merely stretched, so that the supported range
    includes common wide displays.
12. As a maintainer, I want no resolution designated as the Reference
    Deployment or canonical viewport, so that tests describe representative
    coverage rather than a personal target.
13. As a maintainer, I want product source grouped under one source root, so
    that the top level communicates the repository's shape immediately.
14. As a maintainer, I want the bridge to remain a distinct module, so that
    Roon integration and runtime coordination retain their locality.
15. As a maintainer, I want the renderer to remain a distinct module, so that
    native presentation behavior retains its locality.
16. As a maintainer, I want the launcher to remain an explicit module, so that
    the owner-facing command continues to hide runtime composition.
17. As a maintainer, I want schemas and Fixture Scenarios grouped as one shared
    contract module, so that the real seam between the bridge and renderer is
    visible.
18. As a maintainer, I want documentation to remain at the top level, so that
    owner and maintainer guidance is easy to discover.
19. As a maintainer, I want repository scripts to remain at the top level, so
    that maintenance commands remain visible as entrypoints rather than being
    mistaken for runtime implementation.
20. As a maintainer, I want workspace manifests, lockfiles, and tool
    configuration to remain at the root, so that npm, Cargo, formatting, and CI
    continue to have conventional entrypoints.
21. As a developer, I want generated npm and Cargo directories to retain their
    conventional ignored locations, so that cosmetic cleanup does not fight
    the package managers.
22. As a source reader, I want the shared contract seam to remain
    language-neutral, so that neither runtime module owns the other module's
    framework types.
23. As a source reader, I want the reason for the separate Roon bridge and
    native renderer recorded once, so that the architecture is understandable
    without an implementation ledger.
24. As a source reader, I want obvious facts such as the current programming
    languages and transport details left to the code, so that ADRs record only
    durable tradeoffs.
25. As a developer, I want Now Playing terminology used for the Now Playing
    layout, so that implementation vocabulary agrees with the domain glossary.
26. As a developer, I want presentation terminology used for visual capture
    and acceptance workflows, so that a discarded Gallery prototype no longer
    names production behavior.
27. As a fixture author, I want a neutral Speaker System as the sample Tracked
    Output, so that fixtures clearly describe a Roon audio endpoint rather than
    a video display.
28. As a fixture author, I want Living Room as the sample Tracked Zone, so that
    sample data is ordinary and unrelated to the discarded Gallery design
    name.
29. As a fixture author, I want neutral output and zone identifiers, so that
    internal IDs do not preserve obsolete personal terminology.
30. As a maintainer, I want the runnable font-study prototype removed from the
    current tree, so that implemented prior art is not mistaken for supported
    product source.
31. As a maintainer, I want the separate prototype branch left untouched, so
    that branch cleanup remains an independent owner action.
32. As a maintainer, I want durable presentation design guidance without
    prototype history or implementation-ticket narration, so that visual intent
    remains useful after the prototype and local tickets disappear.
33. As a maintainer, I want a self-contained visual-acceptance workflow, so
    that I can review Fixture Scenarios without following links to private
    deployment work.
34. As a maintainer, I want visual captures treated as review artifacts rather
    than golden tests, so that renderer and font differences are judged rather
    than mistaken for deterministic pixel failures.
35. As a README reader, I want links and terminology to remain correct after
    the restructure, so that the repository does not temporarily publish
    broken navigation.
36. As the owner planning a later README rewrite, I want that rewrite kept out
    of this restructure, so that the two efforts retain clear acceptance
    criteria.
37. As the project owner, I want the future `.xinitrc` example reserved for
    getting-started guidance, so that it is not reintroduced as a personal
    deployment recipe during this cleanup.
38. As a prospective owner, I want an MIT license at the repository root, so
    that I can understand the terms under which I may use and modify RoonScape.
39. As a tool inspecting the npm and Rust packages, I want machine-readable MIT
    metadata, so that package metadata agrees with the repository license.
40. As the project owner, I want the Roon extension to publish my GitHub
    noreply address rather than my personal Gmail address, so that the product
    retains accountable authorship without unnecessary personal contact data.
41. As a bug reporter, I want a guided GitHub issue form, so that I provide the
    environment, reproduction, expected result, actual result, and useful logs
    needed for triage.
42. As a feature requester, I want a guided GitHub issue form, so that I
    explain the motivating problem, desired outcome, and alternatives rather
    than prescribing an isolated implementation.
43. As a reporter with an unusual request, I want blank GitHub issues enabled,
    so that the standard forms do not prevent relevant communication.
44. As a maintainer, I want submitted bug and feature forms to receive one
    category and the needs-triage state, so that new work enters the canonical
    triage state machine correctly.
45. As a maintainer, I want blank issues to enter the unlabeled attention
    bucket, so that the triage workflow can classify exceptional requests.
46. As an agent, I want the five canonical triage states mapped to GitHub
    labels, so that issue state remains unambiguous.
47. As an agent, I want every triaged issue to have exactly one category and
    one state, so that conflicting ownership or readiness signals are visible.
48. As a maintainer, I want external pull requests excluded from the request
    triage surface, so that an unsolicited diff is not automatically treated
    as a feature request.
49. As the project owner, I want the existing local specs and tickets retained
    throughout this effort, so that agents can complete the final local work
    before the tracker changes.
50. As the project owner, I want sole control over deleting `.scratch`, so that
    no agent removes the active spec, tickets, or their history prematurely.
51. As the project owner, I want completed historical tickets omitted from
    GitHub Issues, so that the new tracker begins with current work rather than
    a synthetic archive.
52. As a maintainer, I want the existing release packaging and tag workflow
    preserved, so that repository cleanup does not discard already-tested
    release preparation.
53. As the project owner, I want release auditing, stable download URLs, and
    the first published release deferred, so that this effort does not imply a
    release exists.
54. As a maintainer, I want dependency, toolchain, and action versions
    preserved, so that structural changes are not coupled to unrelated
    upgrades.
55. As an owner, I want public CLI behavior, Display Configuration, Roon
    Authorization, and snapshot behavior preserved, so that repository cleanup
    does not alter product use.
56. As a maintainer, I want no compatibility obligation for current source or
    archive paths, so that the repository can adopt its intended structure
    before its first release.
57. As a maintainer, I want the complete repository check suite to pass after
    every path and vocabulary change, so that the restructure remains
    behavior-preserving.
58. As the project owner, I want current-tree cleanup only, so that published
    Git history and commit identities are not rewritten.
59. As the project owner, I want no contributor, security, or community policy
    documents invented before there is a real process to document, so that the
    repository does not make commitments it cannot yet support.
60. As the project owner, I want implementation to stop before commit, push,
    branch deletion, or release publication, so that publication remains an
    explicit owner action.

## Implementation Decisions

- The repository's public posture is an owner-ready open-source project, not a
  community-governed project. It will have an MIT license but no contribution
  guide, security policy, or Code of Conduct in this effort.
- The tracked top-level directory groups will be GitHub metadata,
  documentation, maintainer scripts, and product source. Documentation and
  scripts remain directly discoverable; the source group contains the bridge,
  renderer, launcher, and shared contract modules.
- The bridge and renderer remain distinct modules because they provide
  separate implementations behind the existing process seam. Grouping them
  under the source root must not merge their responsibilities or expose new
  cross-module implementation knowledge.
- The contract module will contain the language-neutral schemas and shared
  Fixture Scenarios. This makes the existing bridge-renderer interface visible
  without adding another layer or adapter.
- The owner-facing launcher moves into the source group. Source-mode commands,
  runtime discovery, packaging, tests, and workspace configuration will be
  updated to resolve the new layout.
- Root manifests, locks, formatting and lint configuration, toolchain pins,
  agent instructions, the domain glossary, and the README remain at the
  repository root.
- Conventional ignored npm, Cargo, and release outputs are not part of the
  tracked-layout objective. Their generated contents are neither preserved as
  source nor reorganized for cosmetic reasons.
- `.scratch` remains the active local issue tracker for this effort. Agents
  must not delete, empty, rename, migrate, or otherwise clean it up. After all
  local tickets are complete, the repository owner will delete it separately.
- No completed local specs or tickets will be recreated as closed GitHub
  issues. Host-specific unfinished work will not be transferred to the public
  tracker.
- The current-tree prototype is removed because its relevant visual choices
  are already implemented. The local and remote prototype branch remain
  untouched.
- Gallery-derived visual terminology is removed from production code, tests,
  scripts, commands, CSS roles, and documentation. The split Now Playing
  composition uses Now Playing terminology; workflows spanning all
  presentations use presentation or visual-acceptance terminology.
- Ordinary sample Roon identities are also generalized. The representative
  Tracked Output is Speaker System, the representative Tracked Zone is Living
  Room, and their internal IDs use corresponding neutral terms. Long-identity
  Fixture Scenarios retain their stress-test length without referring to a
  gallery wall.
- References to the personal Reference Deployment, Intel NUC, private host
  assumptions, guarded tty startup, and a canonical 4K viewport are removed
  from current public source and documentation. OLED-safe product behavior
  remains because it is a product capability rather than a private-host detail.
- The responsive product claim covers common landscape displays at 1280x720
  or larger. Representative coverage includes 1280x720, 1600x1200,
  1920x1200, 2560x1080, and 3840x2160 without designating any one resolution as
  canonical. Portrait and unusually small displays are not supported by this
  effort.
- The durable presentation-design document is self-contained and records only
  current visual intent. It does not preserve prototype chronology, local
  ticket references, or the former Gallery name.
- The visual-acceptance document remains a separate maintainer workflow. It
  explains capture generation, the representative landscape matrix, review
  criteria, and the non-golden nature of the artifacts without a physical-host
  handoff.
- The current README receives correctness edits only: obsolete personal
  deployment material and dead local-ticket or prototype links are removed,
  and paths and commands affected by the restructure are updated. The planned
  owner-focused rewrite, screenshots, getting-started guide, and `.xinitrc`
  example are separate work.
- The repository receives the standard MIT license. Both private npm manifests
  declare MIT, and the Rust renderer package declares MIT and the GitHub
  repository URL for machine-readable tooling.
- The Roon extension retains the owner's name and repository URL but replaces
  the personal Gmail address with
  `5353310+gregbartell@users.noreply.github.com`.
- The existing architectural records are consolidated into one ADR explaining
  the separate Node.js Roon bridge and native renderer. Current language,
  transport, supervision, and repository-layout facts remain visible in code
  rather than being duplicated as architectural decisions.
- The domain context describes RoonScape without calling it a personal display.
  No new domain term is introduced for the removed Reference Deployment or
  Gallery design name.
- The Matt Pocock setup workflow will switch agent-facing tracker guidance to
  GitHub and retain the single-context domain layout. Its default triage-label
  mapping is used unchanged.
- GitHub Issues will be enabled explicitly because the setup workflow does not
  change repository settings. Existing category labels remain. The missing
  canonical state labels are created remotely, while the existing `wontfix`
  label is reused.
- Bug issue forms apply exactly the `bug` category and `needs-triage` state and
  request a summary, environment, reproduction steps, expected behavior,
  actual behavior, and redacted logs.
- Feature issue forms apply exactly the `enhancement` category and
  `needs-triage` state and request the motivating problem, desired outcome, and
  alternatives. Blank issues remain enabled and begin unlabeled.
- Every triaged issue has exactly one of the `bug` or `enhancement` categories
  and exactly one of `needs-triage`, `needs-info`, `ready-for-agent`,
  `ready-for-human`, or `wontfix`. Pull requests are not a request surface.
- Existing release packaging and the tag-triggered GitHub Release workflow are
  retained and updated only as required by source moves. Publishing or
  validating a first release remains separate work.
- Existing dependencies, toolchains, and GitHub Action versions remain pinned.
  No upgrade campaign is bundled into the restructure.
- There is no backward-compatibility obligation for source paths or the
  unpublished release archive's internal paths. Public CLI behavior,
  configuration and authorization locations, the bridge-renderer snapshot
  contract, and viewer-facing behavior remain stable.
- Cleanup applies to the current tree only. Git history is not rewritten, and
  no local or remote branch is deleted.
- The implementation produces local changes and agreed GitHub repository
  settings only. It does not commit, push, tag, or publish a release.

## Testing Decisions

- Good tests observe behavior through an existing module interface or the
  highest available repository seam. Tests should remain stable when source is
  moved or internal implementation names change; assertions that exist only to
  mirror old paths or obsolete vocabulary should be replaced rather than
  layered with compatibility aliases.
- No new product seam is required for this effort. The existing shared
  contract, owner-facing command, package builder, layout policy, capture
  workflow, and repository check command provide sufficient acceptance
  surfaces.
- The repository-wide acceptance seam is the existing complete check command.
  It must run formatting checks, TypeScript and Rust type checks, linters,
  bridge and renderer tests, script tests, packaging tests, and the IPC smoke
  exercise successfully from the reorganized root.
- Workspace tests must prove that npm resolves the relocated bridge workspace,
  Cargo resolves the relocated renderer member, and both toolchains continue
  to use their root manifests and locks.
- Launcher and command tests must prove that source-mode execution still finds
  the built bridge and renderer and that owner-facing setup, ordinary launch,
  help, version, and configuration behavior remain unchanged.
- Packaging tests remain the prior art for verifying that source relocation is
  translated into a runnable archive. They may adopt a new internal archive
  layout, but the built launcher must find its bundled runtime modules and
  assets without depending on source-tree paths.
- Shared schemas and Fixture Scenarios remain the highest cross-language
  contract seam. Existing bridge snapshot tests and renderer snapshot-contract
  tests must consume the relocated shared assets and continue to agree on
  valid and invalid states.
- Renderer layout tests must validate external layout invariants across
  1280x720, 1600x1200, 1920x1200, 2560x1080, and 3840x2160. Useful invariants
  include complete viewport use without unintended letterboxing, bounded and
  readable metadata, contained artwork, stable identities, and OLED-safe
  geometry.
- The existing Now Playing, full-field, metadata, transition, palette,
  diagnostics, inactivity, and feature-integration tests are prior art for
  behavior-preserving visual changes. Their names may change with the new
  vocabulary, but their behavioral coverage remains.
- The visual-capture planner and its tests must cover every maintained Fixture
  Scenario at the agreed representative viewports. Captures remain optional
  human-review artifacts and are not committed or compared pixel-for-pixel in
  CI.
- A human visual review must confirm that the composition remains intentional
  at the minimum, 4:3, 16:10, ultrawide, and high-resolution samples and that no
  viewport is described or treated as the Reference Deployment.
- Static repository checks must find no obsolete Reference Deployment, Intel
  NUC, personal Gmail, Gallery design, prototype, or dead local-ticket
  references in the public source and documentation being cleaned. The active
  `.scratch` effort is excluded because its owner-controlled deletion happens
  later.
- Documentation checks must verify that every retained README and design link
  resolves after the moves and that command examples name current scripts.
- GitHub configuration must be inspected after mutation to verify that Issues
  are enabled and all five canonical state labels exist exactly once. The two
  issue forms must be valid GitHub issue-form YAML and must apply exactly one
  category plus `needs-triage`; blank issues must remain enabled.
- The final worktree audit must distinguish intentional task changes from any
  pre-existing user changes, retain `.scratch`, and confirm that no commit,
  push, tag, release, or branch deletion occurred.

## Out of Scope

- Deleting, emptying, renaming, migrating, or rewriting `.scratch`; only the
  repository owner will remove it after these final local tickets are done.
- Importing completed local specs or tickets into GitHub Issues.
- Tracking personal-host installation, unattended boot acceptance, physical
  OLED calibration, or other work specific to the former Reference Deployment.
- Rewriting Git history or removing already-published historical prose.
- Deleting the local or remote prototype branch.
- The full owner-focused README rewrite, screenshots, stable download command,
  getting-started guide, or `.xinitrc` example.
- Auditing, redesigning, or publishing the first GitHub Release; creating a
  tag; or establishing a stable latest-release URL.
- Updating dependencies, language toolchains, or GitHub Action versions.
- Adding a contribution guide, security policy, Code of Conduct, CLA, DCO, or
  community-governance process.
- Treating external pull requests as feature requests in the triage workflow.
- Supporting portrait displays or landscape displays smaller than 1280x720.
- Changing RoonScape into Roon Control or adding a browser interface, network
  listener, or remote command surface.
- Changing viewer-facing playback behavior, Display Configuration, Roon
  Authorization, XDG storage locations, or the versioned presentation-snapshot
  semantics.
- Preserving compatibility for old source paths or unpublished archive-internal
  paths.
- Committing, pushing, deleting branches, tagging, or publishing a release.

## Further Notes

- This spec and its implementation tickets are intentionally the final local
  tracker effort. GitHub becomes authoritative only after the setup and
  repository configuration work is complete and the owner removes `.scratch`.
- The future README rewrite plan has been moved outside the repository for
  safekeeping. It is not an implementation input beyond the explicit scope
  boundaries recorded here.
- The glossary scope sentence and consolidated architecture decision were
  updated during the design session and are already present in the working
  tree.
- At the time of specification, the GitHub repository is public, GitHub Issues
  are disabled, the default `bug`, `enhancement`, and `wontfix` labels exist,
  and no repository license is detected.
- The literal hostname `roll` is absent from the current tree and published
  main-branch history. The cleanup targets generalized personal-deployment
  assumptions in the current tree rather than a secret-bearing hostname.
