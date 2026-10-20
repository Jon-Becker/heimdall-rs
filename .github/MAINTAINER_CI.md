# Maintainer CI for reviewed pull requests

Ordinary fork pull requests do not receive repository secrets. Approving or
rerunning their normal CI does not change that. **Maintainer CI** is a separate,
manual path for a maintainer who has reviewed and trusts an exact PR commit.

## Run it

1. Review the complete PR head, including workflow/build scripts, dependencies,
   tests, and code that can influence evaluation prompts or reports. Copy its
   full 40-character head SHA.
2. In **Actions → Maintainer CI → Run workflow**, select **main**, enter the PR
   number and reviewed head SHA, and explicitly confirm the trust/cost checkbox.
3. Inspect the run's Linux/macOS release tests, doc tests, coverage comparison,
   and AI evaluation. Download the `maintainer-coverage` and
   `maintainer-eval-results` artifacts for detailed results.

Users with current write, maintain, or admin repository access may dispatch it.
The original actor and rerunning actor are both checked before every execution
job. A closed PR, missing head repository, or changed head SHA is rejected. A
force-push after validation cannot substitute a different commit: checkout uses
the validated repository and exact SHA, never a moving PR ref. Review a changed
head and start a new dispatch rather than reusing an old approval.

## Trust and cost boundary

This is **not a sandbox for untrusted code**. The selected commit can execute
arbitrary code through its build, dependencies, tests, and binary. Running it
intentionally trusts that code with `RPC_URL` and `WS_RPC_URL` in the test/coverage
jobs, and private evaluation-harness access plus `OPENROUTER_API_KEY` in the eval
job. Both base and candidate AI evaluations can incur provider charges; there is
no per-run monetary cap. Generated evaluation content may be sent to the
configured model provider. Do not dispatch on code you would not trust with
these resources.

The evaluation checkout uses the existing `EVAL_REPO_TOKEN`; no new credentials
or permissions are configured. Checkout credentials are not persisted. Code
execution uses read-only GitHub tokens on fresh GitHub-hosted runners, without
shared dependency caches or AWS/OIDC permissions. These choices reduce exposure,
but cannot make malicious approved code safe. Evaluation output and HTML reports
are untrusted data: they are artifacts only, not published to S3 or consumed by
the privileged status job.

Coverage and AI evaluation compare the candidate against the PR's base SHA
pinned by the initial validation job. Coverage preserves the normal workflow's
two-percentage-point regression threshold. The eval harness uses its current
`main`, matching the existing eval workflow.

## Status and scope

Because manual dispatch's native checks are attached to the selected main
commit, a separate no-checkout job posts a **Maintainer CI** status to the exact
reviewed PR SHA. It uses only GitHub job results, never candidate files,
artifacts, or model output. All jobs must succeed; failures, cancellation, and
skips cannot produce a success status. If the PR head changes or permission
validation fails, no new status is posted.

This does not replace or impersonate the existing required checks, fix their
fork-only missing-secret failures, change branch protection, or automatically
approve/merge a PR. Existing automatic CI and the evaluation checkbox workflow
are unchanged. Lint, formatting, and compilation checks continue in ordinary PR
CI and are not duplicated here. Benchmark, release, publishing, and ignored
heavy integration jobs are not part of this manual workflow.

## Validate workflow changes

Run `node --test .github/tests/maintainer-ci.test.cjs` for mocked authorization
and status tests. Also lint the YAML with an up-to-date `actionlint` version
that supports YAML anchors. Never dispatch secret-bearing CI merely to test an
authorization change before reviewing it.
