# Deterministic Recovery Flow (Issue #49)

> Threat model for cross-boundary trust decisions: see [Threat Model](THREAT_MODEL.md).
Recovery checkpoints for multi-step operations that can be interrupted by a
wallet action, API call, webhook, worker, or browser session failing partway
through. The implementation lives in
[`shared::recovery`](../shared/src/recovery.rs).

## Problem

Some steps have **external side effects** (a debit, a notification, a webhook
delivery). Retrying the whole operation after such a step duplicates the side
effect; refusing to retry strands the caller. The checkpoint records exactly
which steps succeeded so a resumed run never repeats a side effect.

## State machine

```
              open_operation
                    │
                    ▼
              ┌────────────┐  complete_step (last step)   ┌───────────┐
              │ InProgress │ ────────────────────────────► │ Completed │
              └─────┬──────┘                              └───────────┘
        fail_step   │   resume
                    ▼     ▲
             ┌──────────────────┐   abandon   ┌────────┐
             │ AwaitingRecovery │ ──────────► │ Failed │
             └──────────────────┘             └────────┘
```

| Transition | Function | Notes |
|---|---|---|
| start | `open_operation` | rejects a duplicate id that is still in flight |
| advance | `complete_step` | must be the step at `cursor`; idempotent if already succeeded |
| interrupt | `fail_step` | moves to `AwaitingRecovery`, records step + error code |
| retry | `resume` | resets only the failed step to `Pending`, keeps succeeded steps |
| give up | `abandon` | marks `Failed`, no further side effects |
| inspect | `next_action` / `diagnostics` / `is_stuck` | user-visible action + maintainer view |

## Why recovery is deterministic

- Steps carry contiguous indices starting at `0`, validated in
  `open_operation`. Any gap is `RecoveryError::InvalidSteps`.
- `complete_step` and `fail_step` only accept the step at `cursor`, so an
  out-of-order call is `StepOutOfOrder` rather than an inconsistent checkpoint.
- `cursor` uniquely identifies the resume point, so `resume` always continues
  at the same step — never skipping or double-running one.

## Idempotency (no duplicate side effects)

`complete_step` on a step that is already `Succeeded` returns the unchanged
checkpoint and does **not** run the step again. Combined with `resume` resetting
only the *failed* step, this means:

- Steps completed before the interruption are never retried.
- Steps that succeeded in a later attempt are not re-applied.

## User-visible next steps

`next_action(env, id)` returns a short symbol callers can surface directly:

| Symbol | Meaning |
|---|---|
| `run` | operation is fresh; run the steps |
| `resume` | interrupted; resume from the checkpoint |
| `done` | completed successfully |
| `support` | abandoned/failed; contact support |
| `unknown` | no checkpoint for this id |

## Maintainer diagnostics

```rust
let d = diagnostics(&env, &op_id).unwrap();
// d.state, d.cursor, d.total, d.succeeded, d.failed, d.pending,
// d.attempts, d.last_error, d.next_action
```

`is_stuck(env, id, now, stale_after)` returns `true` when an `InProgress`
operation has not advanced for `stale_after` units, which is what an alert
should page on.

## Cross-boundary trust decisions

Recovery checkpoints are a trust boundary: the checkpoint is the only
authority on which external side effects have already fired. Any caller,
worker, webhook, or operator that resumes an operation is trusting the
checkpoint to be complete and monotonic. The scenario-based threat model
in [Threat Model](THREAT_MODEL.md) enumerates the abuse cases that cross
this boundary (replayed webhooks, forged `resume` calls, stale checkpoints
after a restore, operator-driven `abandon`) and maps each to a mitigation
and an executable check.

Highest-risk paths and their checks:

- Duplicate side effect on resume — `complete_step` idempotency, covered by
  `cargo test -p shared recovery`.
- Out-of-order advance — `cursor`-gated `complete_step`/`fail_step`,
  covered by the `StepOutOfOrder` tests.
- Stale in-flight operation — `is_stuck` alerting, covered by the
  `stale_after` tests.
- Checkpoint tampering across a restore — see
  [Restore Validation](RESTORE_VALIDATION.md).

## Tests

```bash
cargo test -p shared recovery
```

The suite covers interruption before (`open_operation` + first step), during
(`fail_step` on an external step, then `resume`), and after (idempotent
re-completion, `abandon`, reopen) external side effects.

## Failure modes and recovery

- `RecoveryError::InvalidSteps` — reject the operation; do not open a
  checkpoint with gaps. Roll back by discarding the id and re-issuing steps.
- `StepOutOfOrder` — the caller is racing another resumer. Do not retry
  blindly; re-read `diagnostics` and resume from `cursor`.
- `AwaitingRecovery` with no progress — page on `is_stuck`; operator either
  `resume`s (safe, idempotent) or `abandon`s (terminal, no further side
  effects).
- Checkpoint lost or rolled back by a restore — treat as untrusted; run
  [Restore Validation](RESTORE_VALIDATION.md) before resuming any operation.

## Restore Validation

For checking invariants after a restore or migration, see [Restore Validation](RESTORE_VALIDATION.md).
