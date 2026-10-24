# UI architecture — step plans (ROADMAP M2.6)

Task plans for coding agents, one file per step of
[ROADMAP M2.6](../../../ROADMAP.md#m26--ui-architecture-surfaces-from-actions-settings-model-theme-tokens).
The rules they implement are in [decision 0018](../../decisions/0018-ui-surfaces-from-action-registry.md)
and AGENTS.md section 4, "UI surfaces".

| Step | Plan | Changes appearance | Depends on |
| --- | --- | --- | --- |
| 1 | [Theme tokens](01-theme-tokens.md) | No | — |
| 2 | [Buttons run actions](02-buttons-run-actions.md) | No | — |
| 3 | [Settings model and window](03-settings-model.md) | Yes: settings move to their own window | 2 |
| 4 | [Surfaces as lists](04-surfaces-as-lists.md) | Yes: new toolbar, selection bar, context menu | 1, 2 |
| 5 | [Remove Board tools](05-remove-board-tools.md) | Yes: window gone | 3, 4 |

Steps 1 and 2 are independent and can run in either order. The target layout of
steps 3–5 is the redesign described in [docs/DESIGN.md](../../DESIGN.md) (added by
the design PR).

## Rules for every step

- Follow AGENTS.md. Run the tier-1 checks for changed crates; do not run tests,
  benchmarks or tier-2/3 checks unless the maintainer asks.
- Do not open a pull request and do not wait for CI; the maintainer merges and
  tests through CI.
- Keep board logic, `.moth` format, rendering and memory behaviour unchanged.
- Every new user-visible string goes through `moth_ui::Text` in English and Russian.
- Every new control keeps a touch path (no hover-only or right-click-only access).
- Update `CHANGELOG.md` under "Unreleased" only for steps that change what users see.
- Tick the step in ROADMAP M2.6 when done.
