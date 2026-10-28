# Issue tracker: Local Markdown

Issues and specs use local Markdown. The default location is `.scratch/`;
the registered Git-tracked plans below are explicit exceptions and take precedence.

## Registered Git-tracked plans

| Feature | Authoritative directory | Plan |
| --- | --- | --- |
| `vox-model-refinement` | `docs/plans/vox-model-refinement/` | [Execution plan](../plans/vox-model-refinement/plan.md) |

For a registered feature, read and update its `spec.md` and `issues/<NN>-<slug>.md`
in the authoritative directory. Do not create or execute a duplicate in `.scratch/`.
The old `.scratch/vox-model-refinement/` documents are superseded; only runtime
evidence remains there. Publish new tickets for this feature in the registered directory.

## Conventions

- One feature per directory: `.scratch/<feature-slug>/`
- The spec is `.scratch/<feature-slug>/spec.md`
- Implementation issues are one file per ticket at `.scratch/<feature-slug>/issues/<NN>-<slug>.md`, numbered from `01`, never a single combined tickets file
- Triage state is recorded as a `Status:` line near the top of each issue file (see `triage-labels.md` for the role strings)
- Comments and conversation history append to the bottom of the file under a `## Comments` heading

## When a skill says "publish to the issue tracker"

Create a new file under `.scratch/<feature-slug>/` (creating the directory if needed).

## When a skill says "fetch the relevant ticket"

Read the file at the referenced path. The user will normally pass the path or the issue number directly.

If given a feature name and ticket number, resolve it in that feature's authoritative
directory above, or the default `.scratch/<feature>/issues/` for an unregistered feature.
Read the linked specification, plan and blocking tickets before implementing.
A bare ticket number is ambiguous across features: ask which feature rather than
choosing the first matching file. Record completion and verification evidence in the
same authoritative ticket; never update the superseded copy.

## Wayfinding operations

Used by `/wayfinder`. The **map** is a file with one **child** file per ticket.

- **Map**: `.scratch/<effort>/map.md` (the Notes / Decisions-so-far / Fog body).
- **Child ticket**: `.scratch/<effort>/issues/NN-<slug>.md`, numbered from `01`, with the question in the body. A `Type:` line records the ticket type (`research`/`prototype`/`grilling`/`task`); a `Status:` line records `claimed`/`resolved`.
- **Blocking**: a `Blocked by: NN, NN` line near the top. A ticket is unblocked when every file it lists is `resolved`.
- **Frontier**: scan `.scratch/<effort>/issues/` for files that are open, unblocked, and unclaimed; first by number wins.
- **Claim**: set `Status: claimed` and save before any work.
- **Resolve**: append the answer under an `## Answer` heading, set `Status: resolved`, then append a context pointer (gist + link) to the map's Decisions-so-far in `map.md`.
