'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const { parseReportData, normalizeReports, publishReports, renderBody, escapeName, MAX_BYTES } = require('../scripts/ci-reports.cjs');
const SHA = 'a'.repeat(40);
const BASE = 'b'.repeat(40);
const sample = () => normalizeReports({
  coverageBase: { data: [{ totals: { lines: { percent: 90 } } }] },
  coverageCandidate: { data: [{ totals: { lines: { percent: 87 } } }] },
  evalBase: { example: { cfg: 85, decompilation: 70 } },
  evalCandidate: { example: { cfg: 90, decompilation: 75 } },
  benchmark: [{ name: 'benchmark', baseNs: 20, candidateNs: 21 }],
});
function harness(overrides = {}) {
  const writes = [];
  const notices = [];
  const warnings = [];
  const comments = [];
  const artifacts = ['admin-coverage', 'admin-eval-results', 'admin-criterion-report'].map((name, i) => ({ name: `${name}-2`, id: i + 10, workflow_run: { id: 100 }, expired: false }));
  const run = { id: 100, run_number: 30, run_attempt: 2 };
  const pr = { state: 'open', head: { sha: SHA } };
  const github = { rest: { actions: {
    getWorkflowRun: async () => ({ data: run }), listWorkflowRunArtifacts: function artifactsEndpoint() {},
  }, pulls: { get: async () => ({ data: pr }) }, issues: {
    listComments: function commentsEndpoint() {},
    createComment: async params => { writes.push({ action: 'create', ...params }); },
    updateComment: async params => { writes.push({ action: 'update', ...params }); },
  } }, paginate: async (method, params) => {
    assert.equal(params.per_page, 100);
    if (method === github.rest.actions.listWorkflowRunArtifacts) { assert.equal(params.run_id, 100); return artifacts; }
    assert.equal(method, github.rest.issues.listComments);
    return comments;
  } };
  const options = { github, context: { repo: { owner: 'owner', repo: 'repo' }, runId: 100, serverUrl: 'https://github.com' },
    core: { notice: message => notices.push(message), warning: message => warnings.push(message) },
    env: { REVIEWED_SHA: SHA, BASE_SHA: BASE, PR_NUMBER: '5', RUN_ATTEMPT: '2', REPORT_DATA: JSON.stringify(sample()), COVERAGE_RESULT: 'failure', EVAL_RESULT: 'success', BENCH_RESULT: 'success', ...overrides } };
  return { options, writes, notices, warnings, comments, artifacts, run, pr };
}
function ownedComment(kind, runNumber, attempt, extra = {}) {
  return { id: 50, user: { type: 'Bot', login: 'github-actions[bot]' }, body: `<!-- heimdall-admin-${kind}-report -->\n<!-- heimdall-admin-provenance run=100 number=${runNumber} attempt=${attempt} sha=${SHA} -->\nOld`, ...extra };
}
test('normalizes numeric coverage, base/candidate eval and benchmark rows', () => {
  const data = sample();
  assert.deepEqual(data.coverage, { base: 90, candidate: 87 });
  assert.equal(data.eval.rows[0].baseCfg, 85);
  assert.equal(data.eval.rows[0].decompilation, 75);
  assert.equal(data.benchmark.rows[0].candidateNs, 21);
  assert.deepEqual(parseReportData(JSON.stringify(data)), data);
});
test('does not coerce strings or invent missing metrics', () => {
  const data = normalizeReports({ evalCandidate: { missing: { cfg: '90', decompilation: 101 } } });
  assert.deepEqual(data.coverage, { base: null, candidate: null });
  assert.deepEqual(data.eval.rows[0], { name: 'missing', cfg: null, decompilation: null, baseCfg: null, baseDecompilation: null });
});
test('rejects invalid schemas, out-of-range numbers, duplicate names and excessive input', () => {
  const bad = [null, {}, { ...sample(), extra: 'ignored?' }, { ...sample(), version: 2 }];
  for (const item of bad) assert.throws(() => parseReportData(JSON.stringify(item)));
  for (const value of ['90', -1, 101, {}, true]) {
    const data = sample(); data.coverage.base = value;
    assert.throws(() => parseReportData(JSON.stringify(data)));
  }
  const duplicate = sample(); duplicate.eval.rows.push(duplicate.eval.rows[0]);
  assert.throws(() => parseReportData(JSON.stringify(duplicate)));
  const tooMany = sample(); tooMany.eval.rows = Array.from({ length: 101 }, (_, i) => ({ ...sample().eval.rows[0], name: String(i) }));
  assert.throws(() => parseReportData(JSON.stringify(tooMany)));
  assert.throws(() => parseReportData(' '.repeat(MAX_BYTES + 1)));
  assert.throws(() => parseReportData('{"version":1,"coverage":{"base":1e999,"candidate":5},"eval":{"rows":[]},"benchmark":{"rows":[]}}'));
});
test('neutralizes Markdown, HTML, mentions, URLs and table delimiters', () => {
  const escaped = escapeName('@everyone [click](https://evil.example) | <img> `x`');
  for (const dangerous of ['@everyone', '[click]', 'https://', '|', '<img>', '`']) assert.ok(!escaped.includes(dangerous));
  assert.ok(escaped.includes('＠everyone'));
  const data = sample(); data.eval.rows[0].name = 'bad\nrow';
  assert.throws(() => parseReportData(JSON.stringify(data)));
});
test('renders concrete tables, averages, deltas and GitHub artifact links', async () => {
  const h = harness();
  assert.deepEqual(await publishReports(h.options), { written: 3 });
  assert.equal(h.writes.length, 3);
  assert.match(h.writes[0].body, /90\.00%/);
  assert.match(h.writes[0].body, /-3\.00/);
  assert.match(h.writes[0].body, /dropped by more than 2/);
  assert.match(h.writes[1].body, /\| \*\*Average\*\* \| 85\.00 \| 90\.00 \| 70\.00 \| 75\.00 \|/);
  assert.match(h.writes[2].body, /\+5\.00%/);
  assert.match(h.writes[2].body, /https:\/\/github.com\/owner\/repo\/actions\/runs\/100\/artifacts\/12/);
  for (const write of h.writes) { assert.match(write.body, new RegExp(SHA)); assert.match(write.body, /attempt=2/); }
});
test('missing, expired, duplicate and wrong-attempt artifacts never lend trust to supplied scores', async () => {
  const h = harness();
  h.artifacts[0].expired = true;
  h.artifacts[1].name = 'admin-eval-results-1';
  h.artifacts.push({ ...h.artifacts[2], id: 99 });
  await publishReports(h.options);
  assert.equal(h.writes.length, 3);
  for (const write of h.writes) {
    assert.match(write.body, /No valid numeric measurements/);
    assert.match(write.body, /No matching artifact/);
  }
});
test('artifact metadata from a different run is rejected', async () => {
  const h = harness(); h.artifacts[0].workflow_run.id = 101;
  await publishReports(h.options);
  assert.match(h.writes[0].body, /No matching artifact/);
});
test('malformed report data publishes availability, without leaking parser errors', async () => {
  const h = harness({ REPORT_DATA: '<script>@person</script>' });
  await publishReports(h.options);
  assert.equal(h.warnings.length, 1);
  assert.match(h.writes[0].body, /No valid numeric measurements/);
  assert.ok(!h.writes[0].body.includes('<script>'));
});
test('skips stale head, closed PR and superseded workflow attempts', async () => {
  for (const mutate of [h => { h.pr.head.sha = BASE; }, h => { h.pr.state = 'closed'; }, h => { h.run.run_attempt = 3; }]) {
    const h = harness(); mutate(h);
    assert.deepEqual(await publishReports(h.options), { skipped: true });
    assert.equal(h.writes.length, 0);
  }
});
test('paginates comments and only updates its exact GitHub Actions bot marker', async () => {
  const h = harness();
  h.comments.push(ownedComment('coverage', 30, 1));
  h.comments.push(ownedComment('eval', 30, 1, { id: 51, user: { type: 'User', login: 'human' } }));
  h.comments.push(ownedComment('benchmark', 30, 1, { id: 52, user: { type: 'Bot', login: 'other[bot]' } }));
  await publishReports(h.options);
  assert.deepEqual(h.writes.map(write => write.action), ['update', 'create', 'create']);
  assert.equal(h.writes[0].comment_id, 50);
});
test('never overwrites newer runs, newer attempts or malformed existing provenance', async () => {
  const h = harness();
  h.comments.push(ownedComment('coverage', 31, 1));
  h.comments.push(ownedComment('eval', 30, 3));
  h.comments.push(ownedComment('benchmark', 30, 1, { body: '<!-- heimdall-admin-benchmark-report -->\nunknown' }));
  assert.deepEqual(await publishReports(h.options), { written: 0 });
  assert.equal(h.writes.length, 0);
});
test('checks the live head again immediately before writing', async () => {
  const h = harness();
  h.options.github.paginate = async method => {
    if (method === h.options.github.rest.actions.listWorkflowRunArtifacts) return h.artifacts;
    h.pr.head.sha = BASE; return [];
  };
  const result = await publishReports(h.options);
  assert.equal(result.skipped, true);
  assert.equal(h.writes.length, 0);
});
test('rejects invalid immutable IDs and unsafe context URLs before writing', async () => {
  for (const override of [{ REVIEWED_SHA: 'main' }, { PR_NUMBER: '1;echo' }, { RUN_ATTEMPT: '0' }]) {
    const h = harness(override); await assert.rejects(publishReports(h.options)); assert.equal(h.writes.length, 0);
  }
  const h = harness(); h.options.context.serverUrl = 'https://github.com/evil';
  await assert.rejects(publishReports(h.options));
});
test('output remains bounded and nonfinite timing differences render N/A', () => {
  const metadata = { sha: SHA, baseSha: BASE, runId: 100, runNumber: 30, attempt: 2, runUrl: 'https://github.com/o/r/actions/runs/100', result: 'success', artifactId: 9 };
  const rows = Array.from({ length: 100 }, (_, i) => ({ name: '!'.repeat(150) + i, baseNs: 1e-300, candidateNs: 1e18 }));
  const body = renderBody('benchmark', { rows }, metadata);
  assert.ok(body.length < 60000);
  assert.ok(!body.includes('Infinity'));
  assert.match(body, /Table truncated/);
});
test('same-attempt publication is idempotent', async () => {
  const h = harness();
  await publishReports(h.options);
  h.comments.push(...h.writes.map((write, i) => ({ id: i + 50, user: { type: 'Bot', login: 'github-actions[bot]' }, body: write.body })));
  h.writes.length = 0;
  assert.deepEqual(await publishReports(h.options), { written: 3 });
  assert.equal(h.writes.length, 0);
});
test('unknown result text and artifact-supplied URLs never appear in reports', async () => {
  const h = harness({ EVAL_RESULT: '@everyone https://evil.example' });
  h.artifacts[1].archive_download_url = 'https://evil.example';
  await publishReports(h.options);
  assert.match(h.writes[1].body, /Job result: \*\*unknown\*\*/);
  assert.ok(!h.writes[1].body.includes('evil.example'));
  assert.ok(!h.writes[1].body.includes('@everyone'));
});
