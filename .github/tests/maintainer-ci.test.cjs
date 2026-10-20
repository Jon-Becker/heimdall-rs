const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { test } = require('node:test');

// Test the actual inline workflow code without introducing a runtime dependency.
const workflow = fs.readFileSync(
  path.join(__dirname, '../workflows/maintainer-ci.yml'), 'utf8',
);
const authorization = workflow.split('// BEGIN MAINTAINER AUTHORIZATION\n')[1]
  .split('// END MAINTAINER AUTHORIZATION')[0];
const report = workflow.slice(workflow.lastIndexOf('            const results = ['));
const AsyncFunction = Object.getPrototypeOf(async function () {}).constructor;
const authorize = new AsyncFunction('github', 'context', 'core', 'process', authorization);
const publish = new AsyncFunction('github', 'context', 'process', report);
const sha = 'a'.repeat(40);
const base = 'b'.repeat(40);

function fixture() {
  const outputs = {};
  const checkedUsers = [];
  const statuses = [];
  const permissions = { maintainer: 'write', rerunner: 'admin' };
  const pr = {
    state: 'open',
    base: { sha: base, repo: { full_name: 'Jon-Becker/heimdall-rs' } },
    head: { sha, repo: { full_name: 'contributor/heimdall-rs' } },
  };
  const context = {
    eventName: 'workflow_dispatch',
    ref: 'refs/heads/main',
    actor: 'maintainer',
    repo: { owner: 'Jon-Becker', repo: 'heimdall-rs' },
    serverUrl: 'https://github.com',
    runId: 123,
  };
  const env = {
    PR_NUMBER: '766',
    REVIEWED_SHA: sha,
    TRUST_REVIEWED_CODE: 'true',
    TRIGGERING_ACTOR: 'rerunner',
  };
  const github = { rest: {
    repos: {
      async getCollaboratorPermissionLevel({ username }) {
        checkedUsers.push(username);
        return { data: { permission: permissions[username] } };
      },
      async createCommitStatus(status) { statuses.push(status); },
    },
    pulls: { async get({ pull_number }) {
      assert.equal(pull_number, 766);
      return { data: pr };
    } },
  } };
  const core = {
    setOutput(key, value) { outputs[key] = value; },
    notice() {},
  };
  return {
    outputs, checkedUsers, statuses, permissions, pr, context, env, github,
    run: () => authorize(github, context, core, { env }),
    publish: () => publish(github, context, { env }),
  };
}

for (const permission of ['write', 'maintain', 'admin']) {
  test(`allows ${permission} and returns pinned metadata`, async () => {
    const f = fixture();
    f.permissions.maintainer = permission;
    await f.run();
    assert.deepEqual(f.outputs, {
      head_sha: sha, head_repository: 'contributor/heimdall-rs', base_sha: base,
    });
    assert.deepEqual(f.checkedUsers, ['maintainer', 'rerunner']);
  });
}

for (const permission of ['read', 'triage', 'none', undefined]) {
  for (const actor of ['maintainer', 'rerunner']) {
    test(`rejects ${actor} with ${permission} permission`, async () => {
      const f = fixture();
      f.permissions[actor] = permission;
      await assert.rejects(f.run(), /repository access/);
      assert.deepEqual(f.outputs, {});
    });
  }
}

const rejected = [
  ['automatic event', f => { f.context.eventName = 'pull_request'; }],
  ['non-main branch', f => { f.context.ref = 'refs/heads/feature'; }],
  ['tag named main', f => { f.context.ref = 'refs/tags/main'; }],
  ['unconfirmed trust', f => { f.env.TRUST_REVIEWED_CODE = 'false'; }],
  ['missing trust', f => { delete f.env.TRUST_REVIEWED_CODE; }],
  ['truthy but not true', f => { f.env.TRUST_REVIEWED_CODE = 'yes'; }],
  ['missing rerun actor', f => { delete f.env.TRIGGERING_ACTOR; }],
  ['zero PR', f => { f.env.PR_NUMBER = '0'; }],
  ['negative PR', f => { f.env.PR_NUMBER = '-1'; }],
  ['fractional PR', f => { f.env.PR_NUMBER = '766.1'; }],
  ['unsafe PR integer', f => { f.env.PR_NUMBER = '9007199254740992'; }],
  ['PR injection', f => { f.env.PR_NUMBER = '766; exit 0'; }],
  ['short SHA', f => { f.env.REVIEWED_SHA = 'abcdef0'; }],
  ['non-hex SHA', f => { f.env.REVIEWED_SHA = 'g'.repeat(40); }],
  ['wrong target repository', f => { f.context.repo.owner = 'contributor'; }],
  ['wrong PR base', f => { f.pr.base.repo.full_name = 'other/heimdall-rs'; }],
  ['closed PR', f => { f.pr.state = 'closed'; }],
  ['deleted fork', f => { f.pr.head.repo = null; }],
  ['moved head', f => { f.pr.head.sha = 'c'.repeat(40); }],
];
for (const [name, change] of rejected) {
  test(`rejects ${name} without emitting checkout metadata`, async () => {
    const f = fixture();
    change(f);
    await assert.rejects(f.run());
    assert.deepEqual(f.outputs, {});
  });
}

test('permission API failures fail closed', async () => {
  const f = fixture();
  f.github.rest.repos.getCollaboratorPermissionLevel = async () => {
    throw new Error('API unavailable');
  };
  await assert.rejects(f.run(), /API unavailable/);
  assert.deepEqual(f.outputs, {});
});

test('normalizes a full uppercase SHA', async () => {
  const f = fixture();
  f.env.REVIEWED_SHA = sha.toUpperCase();
  await f.run();
  assert.equal(f.outputs.head_sha, sha);
});

test('each execution job revalidates before checkout', () => {
  for (const job of ['test', 'coverage', 'eval']) {
    const body = workflow.split(`\n  ${job}:\n`)[1].split(/\n  [a-z]+:\n/)[0];
    assert.match(body, /steps:\n      - \*authorize\n      - [*&]checkout-candidate/);
    assert.doesNotMatch(body, /permissions:|rust-cache|id-token:/);
  }
  assert.match(workflow, /permissions:\n  contents: read/);
  assert.doesNotMatch(workflow, /pull_request_target|secrets: inherit/);
  assert.equal((workflow.match(/uses: actions\/checkout@/g) || []).length,
    (workflow.match(/persist-credentials: false/g) || []).length);
});

for (const result of ['success', 'failure', 'cancelled', 'skipped', undefined]) {
  for (const job of ['TEST_RESULT', 'COVERAGE_RESULT', 'EVAL_RESULT']) {
    test(`status handles ${job}=${result}`, async () => {
      const f = fixture();
      Object.assign(f.env, {
        TEST_RESULT: 'success', COVERAGE_RESULT: 'success', EVAL_RESULT: 'success',
      });
      f.env[job] = result;
      await f.publish();
      assert.equal(f.statuses.length, 1);
      assert.equal(f.statuses[0].sha, sha);
      assert.equal(f.statuses[0].context, 'Maintainer CI');
      assert.equal(f.statuses[0].state, result === 'success' ? 'success' : 'failure');
      assert.equal(f.statuses[0].target_url,
        'https://github.com/Jon-Becker/heimdall-rs/actions/runs/123');
    });
  }
}

test('status publisher never checks out code or consumes artifacts', () => {
  const reporter = workflow.split('\n  report:\n')[1];
  assert.doesNotMatch(reporter, /uses: actions\/checkout|download-artifact|secrets\.|run: /);
  assert.match(reporter, /permissions:\n      pull-requests: read\n      statuses: write/);
});
