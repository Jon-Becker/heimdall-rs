# Benchmark report hosting

`.github/workflows/bench.yml` uploads the full Criterion HTML report tree
(`target/criterion/`) to S3 and links it from the benchmark comment on the pull
request.

The upload is **optional**. When the configuration below is missing, or when the
pull request comes from a fork, the workflow skips the upload, says so in the
benchmark comment, and points at the `criterion-report` workflow artifact
instead. Nothing in this document is provisioned by this repository — the bucket,
role, and policies have to be created manually.

## Repository variables

All four are [repository *variables*][vars] (Settings → Secrets and variables →
Actions → Variables), not secrets. No AWS credential is ever stored: the workflow
authenticates with GitHub OIDC.

| Variable | Required | Description |
| --- | --- | --- |
| `BENCH_REPORT_S3_BUCKET` | yes | Bucket that receives the report tree. |
| `BENCH_REPORT_AWS_REGION` | yes | Region the bucket lives in, e.g. `us-east-1`. |
| `BENCH_REPORT_AWS_ROLE_ARN` | yes | Role assumed via OIDC, e.g. `arn:aws:iam::<account-id>:role/<role-name>`. |
| `BENCH_REPORT_BASE_URL` | no | Public origin serving the bucket, e.g. a CloudFront distribution. Defaults to `https://<bucket>.s3.<region>.amazonaws.com`. |

Objects are written under a run-scoped prefix so a canceled or superseded run can
never overwrite a report that is already linked:

```
<base url>/pull/<pr number>/<run id>-<run attempt>/report/index.html
```

`.html`, `.svg`, `.css`, and `.js` objects are uploaded with explicit content
types so the report renders in a browser rather than downloading.

## AWS setup

The bucket needs to be readable by whoever opens the link. Serving it through
CloudFront with an origin access control (and setting `BENCH_REPORT_BASE_URL` to
the distribution domain) keeps the bucket itself private; alternatively, allow
public reads on the `pull/*` prefix. A lifecycle rule expiring `pull/` objects
after ~30 days keeps the bucket from growing without bound.

Register GitHub as an OIDC provider (`token.actions.githubusercontent.com`,
audience `sts.amazonaws.com`), then create the role referenced by
`BENCH_REPORT_AWS_ROLE_ARN` with this trust policy. The `sub` condition limits
the role to pull requests on this repository — fork pull requests run with a
read-only token and no `id-token: write` permission, so they cannot reach it.

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Principal": {
        "Federated": "arn:aws:iam::<account-id>:oidc-provider/token.actions.githubusercontent.com"
      },
      "Action": "sts:AssumeRoleWithWebIdentity",
      "Condition": {
        "StringEquals": {
          "token.actions.githubusercontent.com:aud": "sts.amazonaws.com"
        },
        "StringLike": {
          "token.actions.githubusercontent.com:sub": "repo:Jon-Becker/heimdall-rs:pull_request"
        }
      }
    }
  ]
}
```

The role only ever writes, and only under `pull/`:

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Action": ["s3:PutObject"],
      "Resource": "arn:aws:s3:::<bucket>/pull/*"
    },
    {
      "Effect": "Allow",
      "Action": ["s3:ListBucket"],
      "Resource": "arn:aws:s3:::<bucket>",
      "Condition": { "StringLike": { "s3:prefix": "pull/*" } }
    }
  ]
}
```

[vars]: https://docs.github.com/en/actions/learn-github-actions/variables#defining-configuration-variables-for-multiple-workflows
