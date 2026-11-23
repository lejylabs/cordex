## What and why

<!-- What does this change, and why? Link the issue: "Closes #123". -->

## Checklist

- [ ] `cargo xtask check` passes
- [ ] `cargo xtask features` passes (when features or `cfg` changed)
- [ ] Tests for new behavior (mock tests, no live Discord calls)
- [ ] Public items documented
- [ ] Targets `master`; `CHANGELOG.md` updated under "Unreleased" (user-visible changes, breaks marked **Breaking**)
- [ ] No new dependency, or the reason is given above
