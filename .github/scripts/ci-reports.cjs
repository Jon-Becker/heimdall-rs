'use strict';

// This file must be loaded from the workflow's trusted SHA on a fresh runner.
// Never require files, execute code, or download/extract archives in the writer.
const MAX_BYTES = 128 * 1024;
const MAX_ROWS = 100;
const KINDS = {
  coverage: { title: 'Coverage Report', artifact: 'admin-coverage', result: 'COVERAGE_RESULT' },
  eval: { title: 'AI Evaluation Scores', artifact: 'admin-eval-results', result: 'EVAL_RESULT' },
  benchmark: { title: 'Benchmark Comparison', artifact: 'admin-criterion-report', result: 'BENCH_RESULT' },
};
const emptyReport = () => ({ version: 1, coverage: { base: null, candidate: null }, eval: { rows: [] }, benchmark: { rows: [] } });
const object = value => value !== null && typeof value === 'object' && !Array.isArray(value);
function exactKeys(value, keys) {
  if (!object(value) || Object.keys(value).length !== keys.length ||
      keys.some(key => !Object.hasOwn(value, key))) throw new Error('Invalid report shape');
}
function numeric(value, maximum = 100) {
  return value === null || (typeof value === 'number' && Number.isFinite(value) && value >= 0 && value <= maximum);
}
function nameValid(value) {
  return typeof value === 'string' && value.length > 0 && value.length <= 160 && !/[\x00-\x1f\x7f-\x9f\u202a-\u202e\u2066-\u2069]/u.test(value);
}
function parseReportData(text) {
  if (typeof text !== 'string' || Buffer.byteLength(text, 'utf8') > MAX_BYTES) throw new Error('Report too large or missing');
  const value = JSON.parse(text);
  exactKeys(value, ['version', 'coverage', 'eval', 'benchmark']);
  if (value.version !== 1) throw new Error('Unsupported report version');
  exactKeys(value.coverage, ['base', 'candidate']);
  if (!numeric(value.coverage.base) || !numeric(value.coverage.candidate)) throw new Error('Invalid coverage');
  for (const kind of ['eval', 'benchmark']) {
    exactKeys(value[kind], ['rows']);
    const rows = value[kind].rows;
    if (!Array.isArray(rows) || rows.length > MAX_ROWS) throw new Error('Invalid report rows');
    const names = new Set();
    for (const row of rows) {
      const fields = kind === 'eval' ? ['cfg', 'decompilation', 'baseCfg', 'baseDecompilation'] : ['baseNs', 'candidateNs'];
      exactKeys(row, ['name', ...fields]);
      if (!nameValid(row.name) || names.has(row.name)) throw new Error('Invalid report name');
      names.add(row.name);
      if (fields.some(field => !numeric(row[field], kind === 'eval' ? 100 : 1e18))) throw new Error('Invalid report number');
    }
  }
  return value;
}
function normalizeReports({ coverageBase, coverageCandidate, evalBase, evalCandidate, benchmark } = {}) {
  const result = emptyReport();
  const score = (value, maximum = 100) => numeric(value, maximum) && value !== null ? value : null;
  result.coverage.base = score(coverageBase?.data?.[0]?.totals?.lines?.percent);
  result.coverage.candidate = score(coverageCandidate?.data?.[0]?.totals?.lines?.percent);
  const base = object(evalBase) ? evalBase : {};
  const candidate = object(evalCandidate) ? evalCandidate : {};
  const names = [...new Set([...Object.keys(base), ...Object.keys(candidate)])].filter(nameValid).sort().slice(0, MAX_ROWS);
  result.eval.rows = names.map(name => ({
    name, cfg: score(candidate[name]?.cfg), decompilation: score(candidate[name]?.decompilation),
    baseCfg: score(base[name]?.cfg), baseDecompilation: score(base[name]?.decompilation),
  }));
  const seen = new Set();
  result.benchmark.rows = (Array.isArray(benchmark) ? benchmark : []).filter(row => {
    if (!object(row) || !nameValid(row.name) || seen.has(row.name)) return false;
    seen.add(row.name);
    return true;
  }).slice(0, MAX_ROWS).map(row => ({ name: row.name, baseNs: score(row.baseNs, 1e18), candidateNs: score(row.candidateNs, 1e18) }));
  return parseReportData(JSON.stringify(result));
}
// Names are data, not Markdown, HTML, links, or mention targets. Encode punctuation
// before Markdown parsing; replace @ outright so it cannot notify users.
function escapeName(value) {
  return value.replace(/@/g, '＠').replace(/[^\p{L}\p{N} ＠]/gu, char => `&#${char.codePointAt(0)};`);
}
const number = value => value === null ? 'N/A' : value.toFixed(2);
const difference = (base, candidate, percent = false) => {
  if (base === null || candidate === null || (percent && base === 0)) return 'N/A';
  const diff = percent ? (candidate / base - 1) * 100 : candidate - base;
  if (!Number.isFinite(diff)) return 'N/A';
  return `${diff >= 0 ? '+' : ''}${diff.toFixed(2)}${percent ? '%' : ''}`;
};
function mean(rows, field) {
  const values = rows.map(row => row[field]).filter(value => value !== null);
  return values.length ? values.reduce((sum, value) => sum + value, 0) / values.length : null;
}
function renderBody(kind, data, metadata) {
  const { sha, baseSha, runId, runNumber, attempt, runUrl, artifactId, result } = metadata;
  const marker = `<!-- heimdall-admin-${kind}-report -->`;
  const provenance = `<!-- heimdall-admin-provenance run=${runId} number=${runNumber} attempt=${attempt} sha=${sha} -->`;
  const lines = [marker, provenance, `## ${KINDS[kind].title}`, '', `Reviewed commit: \`${sha}\``, `Base commit: \`${baseSha}\``,
    `Job result: **${result}**. [Workflow run, attempt ${attempt}](${runUrl}/attempts/${attempt}).`, '',
    'Results below are untrusted CI measurements, not an independent verification of the changes.', ''];
  if (kind === 'coverage' && (data.base !== null || data.candidate !== null)) {
    lines.push('| Metric | Value |', '| --- | ---: |', `| Base | ${number(data.base)}% |`,
      `| Candidate | ${number(data.candidate)}% |`, `| Change (percentage points) | ${difference(data.base, data.candidate)} |`);
    if (data.base !== null && data.candidate !== null && data.candidate - data.base < -2) lines.push('', '**Coverage dropped by more than 2 percentage points.**');
  } else if (kind === 'eval' && data.rows.length) {
    lines.push('| Test case | Base CFG | Candidate CFG | Base decompilation | Candidate decompilation |', '| --- | ---: | ---: | ---: | ---: |');
    for (const row of data.rows) {
      if (lines.join('\n').length > 48000) { lines.push('', 'Table truncated; see the artifact for all rows.'); break; }
      lines.push(`| ${escapeName(row.name)} | ${number(row.baseCfg)} | ${number(row.cfg)} | ${number(row.baseDecompilation)} | ${number(row.decompilation)} |`);
    }
    lines.push(`| **Average** | ${number(mean(data.rows, 'baseCfg'))} | ${number(mean(data.rows, 'cfg'))} | ${number(mean(data.rows, 'baseDecompilation'))} | ${number(mean(data.rows, 'decompilation'))} |`);
  } else if (kind === 'benchmark' && data.rows.length) {
    lines.push('| Benchmark | Base mean (ns) | Candidate mean (ns) | Change |', '| --- | ---: | ---: | ---: |');
    for (const row of data.rows) {
      if (lines.join('\n').length > 48000) { lines.push('', 'Table truncated; see the artifact for all rows.'); break; }
      lines.push(`| ${escapeName(row.name)} | ${number(row.baseNs)} | ${number(row.candidateNs)} | ${difference(row.baseNs, row.candidateNs, true)} |`);
    }
    lines.push('', 'Positive timing changes are slower; negative changes are faster. These point estimates do not establish statistical significance.');
  } else lines.push('No valid numeric measurements are available for this run attempt.');
  lines.push('', artifactId
    ? `[Download ${KINDS[kind].artifact} artifact](${runUrl}/artifacts/${artifactId}). HTML reports, if present, are inside the artifact and remain untrusted.`
    : 'No matching artifact is available for this run attempt.');
  if ((kind === 'eval' || kind === 'benchmark') && data.rows.length === MAX_ROWS) lines.push('', `Showing at most ${MAX_ROWS} rows; see the artifact for the complete report.`);
  return lines.join('\n');
}
function integer(value) {
  const text = String(value);
  if (!/^[1-9]\d*$/.test(text) || !Number.isSafeInteger(Number(text))) throw new Error('Invalid numeric identifier');
  return Number(text);
}
function provenance(body) {
  const match = /^<!-- heimdall-admin-provenance run=(\d+) number=(\d+) attempt=(\d+) sha=([a-f0-9]{40}) -->$/m.exec(body || '');
  if (!match) return null;
  try { return { runId: integer(match[1]), runNumber: integer(match[2]), attempt: integer(match[3]), sha: match[4] }; } catch { return null; }
}
async function publishReports({ github, context, core, env = process.env }) {
  const sha = env.REVIEWED_SHA;
  const baseSha = env.BASE_SHA;
  if (!/^[a-f0-9]{40}$/.test(sha || '') || !/^[a-f0-9]{40}$/.test(baseSha || '')) throw new Error('Missing immutable report SHAs');
  const prNumber = integer(env.PR_NUMBER);
  const runId = integer(context.runId);
  const attempt = integer(env.RUN_ATTEMPT);
  const server = new URL(context.serverUrl);
  if (server.protocol !== 'https:' || server.username || server.password || server.pathname !== '/' || server.search || server.hash) throw new Error('Invalid GitHub server');
  for (const part of [context.repo.owner, context.repo.repo]) if (!/^[a-zA-Z0-9_.-]+$/.test(part)) throw new Error('Invalid repository');
  const runUrl = `${server.origin}/${context.repo.owner}/${context.repo.repo}/actions/runs/${runId}`;
  let report;
  try { report = parseReportData(env.REPORT_DATA); } catch { core.warning('Report data was unavailable or invalid; publishing availability only.'); report = emptyReport(); }
  const getRun = async () => (await github.rest.actions.getWorkflowRun({ ...context.repo, run_id: runId })).data;
  const run = await getRun();
  const runNumber = integer(run.run_number);
  const current = async () => {
    const [pr, latest] = await Promise.all([
      github.rest.pulls.get({ ...context.repo, pull_number: prNumber }).then(response => response.data), getRun(),
    ]);
    return pr.state === 'open' && pr.head?.sha === sha && latest.id === runId && latest.run_attempt === attempt && latest.run_number === runNumber;
  };
  if (!await current()) { core.notice('Skipping stale CI report.'); return { skipped: true }; }
  const artifacts = await github.paginate(github.rest.actions.listWorkflowRunArtifacts, { ...context.repo, run_id: runId, per_page: 100 });
  const bot = comment => comment.user?.type === 'Bot' && comment.user?.login === 'github-actions[bot]';
  let written = 0;
  for (const [kind, config] of Object.entries(KINDS)) {
    const marker = `<!-- heimdall-admin-${kind}-report -->`;
    const matches = artifacts.filter(item => item.name === `${config.artifact}-${attempt}` && !item.expired &&
      (!item.workflow_run || item.workflow_run.id === runId) && Number.isSafeInteger(item.id) && item.id > 0);
    const artifactId = matches.length === 1 ? matches[0].id : null;
    const data = artifactId ? report[kind] : emptyReport()[kind];
    const result = ['success', 'failure', 'cancelled', 'skipped'].includes(env[config.result]) ? env[config.result] : 'unknown';
    const body = renderBody(kind, data, { sha, baseSha, runId, runNumber, attempt, runUrl, artifactId, result });
    const comments = await github.paginate(github.rest.issues.listComments, { ...context.repo, issue_number: prNumber, per_page: 100 });
    const owned = comments.filter(comment => bot(comment) && comment.body?.split('\n')[0] === marker);
    // Fail closed on legacy/malformed markers. Never replace a human's comment,
    // a different bot's marker, or a report from a later run/attempt.
    if (owned.some(comment => {
      const old = provenance(comment.body);
      return !old || old.runNumber > runNumber || (old.runNumber === runNumber && (old.runId !== runId || old.attempt > attempt));
    })) { core.notice(`Skipping superseded ${kind} report.`); continue; }
    if (!await current()) { core.notice('Skipping stale CI report.'); return { skipped: true, written }; }
    const existing = owned.sort((a, b) => b.id - a.id)[0];
    if (existing) {
      if (existing.body !== body) await github.rest.issues.updateComment({ ...context.repo, comment_id: existing.id, body });
    } else await github.rest.issues.createComment({ ...context.repo, issue_number: prNumber, body });
    written++;
  }
  return { written };
}
module.exports = { parseReportData, normalizeReports, publishReports, renderBody, escapeName, provenance, MAX_BYTES };
