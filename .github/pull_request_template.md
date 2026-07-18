## Outcome

<!-- Describe the user-visible result and root cause. -->

## Impact matrix

<!-- Mark every row: changed, verified unaffected, or not applicable. Include evidence for verified-unaffected claims. -->

| Surface               | Impact | Evidence |
| --------------------- | ------ | -------- |
| API                   |        |          |
| Database / migrations |        |          |
| Shared contracts      |        |          |
| Web                   |        |          |
| iOS                   |        |          |
| macOS                 |        |          |
| Auth / attestation    |        |          |
| Offline / sync        |        |          |
| External providers    |        |          |
| Deployment / CI       |        |          |

## End-to-end verification

<!-- Trace and test: interaction → client state → request/auth → API/database/provider → response → rendered result. -->

- Workflow exercised:
- Regression test that fails on the old behavior:
- Exact commands and results:
- CI jobs inspected:

## Compatibility, migration, and risk

- API compatibility:
- Data migration / backfill:
- Security / privacy:
- Residual risk or untested boundary:

## Completion checklist

- [ ] I re-read the request and checked the final diff against it.
- [ ] I inspected sibling callers and retry/error paths.
- [ ] I verified web and iOS/macOS parity, or documented why a client is unaffected.
- [ ] I tested every changed package and inspected required CI results.
- [ ] I verified current primary documentation for external-provider contract changes.
- [ ] I kept this PR in draft if an essential check is still pending.
