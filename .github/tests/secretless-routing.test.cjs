'use strict';
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const workflow = name => fs.readFileSync(path.join(__dirname, '../workflows', name + '.yml'), 'utf8');

test('secretless PR test selection includes new tests and excludes only exact live-RPC names', () => {
  const text = workflow('tests');
  const step = text.split('- name: Run secretless PR tests')[1].split('- name: Run Tests')[0];
  assert.match(step, /head\.repo\.full_name != github\.repository \|\| github\.actor == 'dependabot\[bot\]'/);
  assert.match(step, /RPC_URL: ''/);
  assert.match(step, /WS_RPC_URL: ''/);
  assert.match(step, /not \(test\(=/);
  const exclusions = [...step.matchAll(/test\(=([^)]*)\)/g)].map(match => match[1]);
  assert.equal(exclusions.length, 28);
  assert.equal(new Set(exclusions).size, 28);
  assert.ok(exclusions.every(name => /^[a-zA-Z0-9_:]+$/.test(name)));
  assert.ok(exclusions.includes('ether::rpc::tests::test_chain_id_with_ws_rpc'));
  assert.ok(exclusions.includes('integration_tests::test_inspect_create'));
  assert.doesNotMatch(step, /binary\(|package\(/);
  assert.match(step, /cargo nextest run --no-fail-fast --release -E "\$SECRETLESS_FILTER"/);
});

test('credentialed ordinary coverage and eval comments exclude forks and Dependabot', () => {
  for (const name of ['coverage', 'eval']) {
    assert.match(workflow(name), /head\.repo\.full_name == github\.repository && github\.actor != 'dependabot\[bot\]'/);
  }
});

test('benchmark forks use a read-only comparison, not the comment-writing action', () => {
  const text = workflow('bench');
  assert.match(text, /head\.repo\.full_name != github\.repository \|\| github\.actor == 'dependabot\[bot\]'/);
  const action = text.indexOf('uses: boa-dev/criterion-compare-action');
  assert.ok(action > 0);
  assert.match(text.slice(Math.max(0, action - 350), action), /github\.actor != 'dependabot\[bot\]'/);
  const cleanup = text.split('- name: Delete old benchmark comments')[1];
  assert.match(cleanup, /head\.repo\.full_name == github\.repository && github\.actor != 'dependabot\[bot\]'/);
});
