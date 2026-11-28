const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { test } = require('node:test');

// Execute the actual inline workflow code with mocked APIs, without dependencies.
const workflow = fs.readFileSync(
  path.join(__dirname, '../workflows/maintainer-ci.yml'), 'utf8',
);
const authorization = workflow.split('// BEGIN MAINTAINER AUTHORIZATION\n')[1]
  .split('// END MAINTAINER AUTHORIZATION')[0];
const report = workflow.slice(workflow.lastIndexOf('            const results = [')).split('\n  prepare-reports:')[0];
const AsyncFunction = Object.getPrototypeOf(async function () {}).constructor;
const authorize = new AsyncFunction('github', 'context', 'core', 'process', authorization);
const publish = new AsyncFunction('github', 'context', 'process', report);
const sha = 'a'.repeat(40);
const base = 'b'.repeat(40);
const earlier = '2026-10-20T10:00:00Z';
const approvedAt = '2026-10-20T10:00:01Z';
const later = '2026-10-20T10:00:02Z';

function fixture() {
  const outputs = {};
  const checkedUsers = [];
  const statuses = [];
  const permissions = { administrator: 'admin', rerunner: 'admin' };
  const headRepository = {
    id: 200, full_name: 'contributor/heimdall-rs',
    owner: { login: 'contributor' }, name: 'heimdall-rs', pushed_at: earlier,
  };
  const pr = {
    state: 'open',
    base: { sha: base, repo: { full_name: 'Jon-Becker/heimdall-rs' } },
    head: { sha, ref: 'patch', repo: structuredClone(headRepository) },
  };
  const command = {
    id: 456, body: '/ok-to-test', user: { id: 10, login: 'administrator' },
    created_at: approvedAt, updated_at: approvedAt,
  };
  const liveComment = structuredClone(command);
  const context = {
    eventName: 'issue_comment', ref: 'refs/heads/main', actor: 'administrator',
    job: 'authorize', repo: { owner: 'Jon-Becker', repo: 'heimdall-rs' },
    serverUrl: 'https://github.com', runId: 123,
    payload: { action: 'created', issue: { number: 766, pull_request: {} }, comment: command },
  };
  const env = { TRIGGERING_ACTOR: 'rerunner', REVIEWED_SHA: sha };
  const target = { default_branch: 'main' };
  const activity = [{
    after: sha, before: base, ref: 'refs/heads/patch', timestamp: earlier, activity_type: 'push',
  }];
  const currentRef = { object: { sha } };
  let prReads = 0;
  let mutateSecondRead = () => {};
  const github = { rest: {
    repos: {
      async getCollaboratorPermissionLevel({ username }) {
        checkedUsers.push(username);
        return { data: { permission: permissions[username] } };
      },
      async get({ owner, repo }) {
        assert.equal(repo, 'heimdall-rs');
        return { data: owner === 'Jon-Becker' ? target : headRepository };
      },
      async createCommitStatus(status) { statuses.push(status); },
    },
    pulls: { async get({ pull_number }) {
      assert.equal(pull_number, 766);
      if (++prReads === 2) mutateSecondRead(pr);
      return { data: structuredClone(pr) };
    } },
    issues: { async getComment({ comment_id }) {
      assert.equal(comment_id, command.id);
      return { data: liveComment };
    } },
    git: { async getRef(args) {
      assert.deepEqual(args, { owner: 'contributor', repo: 'heimdall-rs', ref: 'heads/patch' });
      return { data: currentRef };
    } },
  }, async request(route, args) {
    assert.equal(route, 'GET /repos/{owner}/{repo}/activity');
    assert.deepEqual(args, {
      owner: 'contributor', repo: 'heimdall-rs', ref: 'refs/heads/patch', direction: 'desc', per_page: 1,
      headers: { 'X-GitHub-Api-Version': '2022-11-28', 'Cache-Control': 'no-cache' },
    });
    return { data: activity };
  } };
  const core = { setOutput(key, value) { outputs[key] = value; }, notice() {} };
  return {
    outputs, checkedUsers, statuses, permissions, pr, command, liveComment, context,
    env, github, target, activity, headRepository, currentRef,
    onSecondRead(fn) { mutateSecondRead = fn; },
    run: () => authorize(github, context, core, { env }),
    publish: () => publish(github, context, { env }),
  };
}

test('admin command returns exact pinned metadata', async () => {
  const f = fixture();
  await f.run();
  assert.deepEqual(f.outputs, {
    head_sha: sha, head_repository: 'contributor/heimdall-rs', base_sha: base,
  });
  assert.deepEqual(f.checkedUsers, ['administrator', 'rerunner']);
});

test('execution jobs revalidate their original snapshot', async () => {
  const f = fixture();
  f.context.job = 'test';
  await f.run();
  assert.equal(f.outputs.head_sha, sha);
});

for (const permission of ['write', 'maintain', 'read', 'triage', 'none', undefined]) {
  for (const actor of ['administrator', 'rerunner']) {
    test(`rejects ${actor} with ${permission} permission, including revoked admin access`, async () => {
      const f = fixture();
      f.permissions[actor] = permission;
      await assert.rejects(f.run(), /admin repository access/);
      assert.deepEqual(f.outputs, {});
    });
  }
}

const rejected = [
  ['automatic PR event', f => { f.context.eventName = 'pull_request'; }],
  ['manual dispatch', f => { f.context.eventName = 'workflow_dispatch'; }],
  ['edited comment event', f => { f.context.payload.action = 'edited'; }],
  ['deleted comment event', f => { f.context.payload.action = 'deleted'; }],
  ['ordinary issue', f => { delete f.context.payload.issue.pull_request; }],
  ['non-default branch', f => { f.context.ref = 'refs/heads/feature'; }],
  ['tag named main', f => { f.context.ref = 'refs/tags/main'; }],
  ['changed default branch', f => { f.target.default_branch = 'new-main'; }],
  ['missing rerun actor', f => { delete f.env.TRIGGERING_ACTOR; }],
  ['different requesting actor', f => { f.context.actor = 'somebody'; }],
  ['zero PR', f => { f.context.payload.issue.number = 0; }],
  ['unsafe PR integer', f => { f.context.payload.issue.number = 9007199254740992; }],
  ['PR injection', f => { f.context.payload.issue.number = '766; exit 0'; }],
  ['missing comment author ID', f => { delete f.command.user.id; }],
  ['impossible date', f => { f.activity[0].timestamp = '2026-02-31T00:00:00Z'; }],
  ['invalid comment ID', f => { f.command.id = -1; }],
  ['quoted command', f => { f.command.body = '> /ok-to-test'; }],
  ['command suffix', f => { f.command.body = '/ok-to-test please'; }],
  ['command plus shell injection', f => { f.command.body = '/ok-to-test\necho "$TOKEN"'; }],
  ['edited command', f => { f.liveComment.updated_at = later; }],
  ['revoked command', f => { f.liveComment.body = 'Do not run this'; }],
  ['replacement comment author', f => { f.liveComment.user.id = 11; }],
  ['changed comment creation time', f => { f.liveComment.created_at = later; }],
  ['short head SHA', f => { f.pr.head.sha = 'abcdef0'; }],
  ['invalid base SHA', f => { f.pr.base.sha = 'not-a-sha'; }],
  ['wrong target repository', f => { f.context.repo.owner = 'contributor'; }],
  ['wrong PR base', f => { f.pr.base.repo.full_name = 'other/heimdall-rs'; }],
  ['closed PR', f => { f.pr.state = 'closed'; }],
  ['deleted fork', f => { f.pr.head.repo = null; }],
  ['new head on failed-job rerun', f => { f.context.job = 'test'; f.env.REVIEWED_SHA = 'c'.repeat(40); }],
  ['missing snapshot on execution job', f => { f.context.job = 'test'; delete f.env.REVIEWED_SHA; }],
  ['missing activity history', f => { f.activity.length = 0; }],
  ['stale activity for a different SHA', f => { f.activity[0].after = base; }],
  ['activity for a different branch', f => { f.activity[0].ref = 'refs/heads/other'; }],
  ['branch deletion', f => { f.activity[0].activity_type = 'branch_deletion'; }],
  ['unknown activity type', f => { f.activity[0].activity_type = 'future_type'; }],
  ['later push', f => { f.activity[0].timestamp = later; }],
  ['same-second push', f => { f.activity[0].timestamp = approvedAt; }],
  ['force-push back to old SHA after command', f => {
    f.activity[0].timestamp = later; f.activity[0].activity_type = 'force_push';
  }],
  ['branch recreation after command', f => {
    f.activity[0].timestamp = later; f.activity[0].activity_type = 'branch_creation';
  }],
  ['missing activity timestamp', f => { delete f.activity[0].timestamp; }],
  ['invalid activity timestamp', f => { f.activity[0].timestamp = 'yesterday'; }],
  ['invalid approval timestamp', f => { f.command.created_at = 'yesterday'; }],
  ['later unrelated repository push', f => { f.headRepository.pushed_at = later; }],
  ['same-second repository push', f => { f.headRepository.pushed_at = approvedAt; }],
  ['missing repository push timestamp', f => { f.headRepository.pushed_at = null; }],
  ['replaced fork repository', f => { f.headRepository.id = 201; }],
  ['head ref changed between reads', f => { f.currentRef.object.sha = base; }],
  ['PR head changed between reads', f => { f.onSecondRead(pr => { pr.head.sha = base; }); }],
  ['PR branch changed between reads', f => { f.onSecondRead(pr => { pr.head.ref = 'other'; }); }],
  ['PR fork changed between reads', f => { f.onSecondRead(pr => { pr.head.repo.id = 201; }); }],
  ['PR closed between reads', f => { f.onSecondRead(pr => { pr.state = 'closed'; }); }],
];
for (const [name, change] of rejected) {
  test(`rejects ${name} without emitting checkout metadata`, async () => {
    const f = fixture();
    change(f);
    await assert.rejects(f.run());
    assert.deepEqual(f.outputs, {});
  });
}

for (const method of ['permission', 'comment', 'activity', 'repository', 'ref', 'pull']) {
  test(`${method} API failure fails closed`, async () => {
    const f = fixture();
    const fail = async () => { throw new Error('API unavailable'); };
    if (method === 'permission') f.github.rest.repos.getCollaboratorPermissionLevel = fail;
    if (method === 'comment') f.github.rest.issues.getComment = fail;
    if (method === 'activity') f.github.request = fail;
    if (method === 'repository') f.github.rest.repos.get = fail;
    if (method === 'ref') f.github.rest.git.getRef = fail;
    if (method === 'pull') f.github.rest.pulls.get = fail;
    await assert.rejects(f.run(), /API unavailable/);
    assert.deepEqual(f.outputs, {});
  });
}

test('backdated commits cannot disguise a later server-recorded push', async () => {
  const f = fixture();
  f.pr.head.commit = { author: { date: '2000-01-01T00:00:00Z' }, committer: { date: '2000-01-01T00:00:00Z' } };
  f.activity[0].timestamp = later;
  await assert.rejects(f.run(), /before this command/);
});

test('never scans backward for an older activity with the same SHA', async () => {
  const f = fixture();
  f.activity.unshift({ ...f.activity[0], timestamp: later, activity_type: 'force_push' });
  await assert.rejects(f.run(), /before this command/);
});

test('old branch creation qualifies when no subsequent mutation occurred', async () => {
  const f = fixture();
  f.activity[0].activity_type = 'branch_creation';
  await f.run();
  assert.equal(f.outputs.head_sha, sha);
});

test('each execution job revalidates before checkout and has read-only credentials', () => {
  for (const job of ['quality', 'validation', 'test', 'coverage', 'bench', 'eval']) {
    const body = workflow.split(`\n  ${job}:\n`)[1].split(/\n  [a-z]+:\n/)[0];
    assert.match(body, /- [*&]revalidate/);
    assert.match(body, /- [*&]checkout-candidate/);
    assert.ok(body.indexOf('revalidate') < body.indexOf('checkout-candidate'));
    assert.doesNotMatch(body, /permissions:|rust-cache|id-token:/);
  }
  assert.match(workflow, /permissions:\n  contents: read/);
  assert.doesNotMatch(workflow, /pull_request_target|workflow_dispatch|secrets: inherit|actions\/cache/);
  assert.equal((workflow.match(/uses: actions\/checkout@/g) || []).length,
    (workflow.match(/persist-credentials: false/g) || []).length);
});

const resultVariables = ['APPROVAL_RESULT', 'VALIDATION_RESULT', 'QUALITY_RESULT', 'BENCH_RESULT', 'TEST_RESULT', 'COVERAGE_RESULT', 'EVAL_RESULT'];
for (const result of ['success', 'failure', 'cancelled', 'skipped', undefined]) {
  for (const job of resultVariables) {
    test(`status handles ${job}=${result}`, async () => {
      const f = fixture();
      for (const variable of resultVariables) f.env[variable] = 'success';
      f.env[job] = result;
      await f.publish();
      assert.equal(f.statuses.length, 1);
      assert.equal(f.statuses[0].sha, sha);
      assert.equal(f.statuses[0].context, 'Admin-approved CI');
      assert.equal(f.statuses[0].state, result === 'success' ? 'success' : 'failure');
      assert.equal(f.statuses[0].target_url,
        'https://github.com/Jon-Becker/heimdall-rs/actions/runs/123');
    });
  }
}

test('status publisher never checks out code or consumes artifacts', () => {
  const reporter = workflow.split('\n  report:\n')[1].split('\n  prepare-reports:')[0];
  assert.doesNotMatch(reporter, /uses: actions\/checkout|download-artifact|secrets\.|run: /);
  assert.match(reporter, /permissions:\n      contents: read\n      pull-requests: read\n      statuses: write/);
});

test('stale approval becomes failure on the original SHA instead of stranded pending', async () => {
  const f = fixture();
  for (const variable of resultVariables) f.env[variable] = 'success';
  f.context.job = 'report';
  f.headRepository.pushed_at = later;
  await assert.rejects(f.run());
  f.env.APPROVAL_RESULT = 'failure';
  await f.publish();
  assert.equal(f.statuses[0].state, 'failure');
  assert.equal(f.statuses[0].sha, sha);
  const reporter = workflow.split('\n  report:\n')[1].split('\n  prepare-reports:')[0];
  assert.match(reporter, /id: revalidate\n        continue-on-error: true/);
  assert.match(reporter, /name: Publish separate Admin-approved CI status\n        if: \$\{\{ always\(\) \}\}/);
});

test('an administrator can approve their own PR without a review requirement', async () => {
  const f = fixture();
  f.pr.user = { login: 'administrator' };
  await f.run();
  assert.equal(f.outputs.head_sha, sha);
});
