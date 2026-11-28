'use strict';
const fs = require('node:fs');
const path = require('node:path');
const { normalizeReports } = require('./ci-reports.cjs');

// Runs only on the separate read-only reporting runner. No artifact is executed.
// Downloaded paths never determine output paths, code imports, or shell commands.
function readJson(root, relative) {
  const base = path.resolve(root);
  if (fs.existsSync(base) && fs.lstatSync(base).isSymbolicLink()) throw new Error('Report roots cannot be symlinks');
  let current = base;
  for (const part of relative.split('/')) {
    if (!part || part === '.' || part === '..') throw new Error('Invalid report path');
    current = path.join(current, part);
    if (!fs.existsSync(current)) return null;
    if (fs.lstatSync(current).isSymbolicLink()) throw new Error('Report symlinks are forbidden');
  }
  const stat = fs.statSync(current);
  if (!stat.isFile() || stat.size > 8 * 1024 * 1024) throw new Error('Report is not a bounded regular file');
  return JSON.parse(fs.readFileSync(current, 'utf8'));
}
function benchmarkRows(root) {
  if (!fs.existsSync(root)) return [];
  if (fs.lstatSync(root).isSymbolicLink()) throw new Error('Benchmark root cannot be a symlink');
  const rows = [];
  let visited = 0;
  function walk(relative, depth) {
    if (depth > 12) throw new Error('Benchmark nesting too deep');
    for (const entry of fs.readdirSync(path.join(root, relative), { withFileTypes: true })) {
      if (++visited > 20000) throw new Error('Too many benchmark files');
      if (entry.isSymbolicLink()) throw new Error('Benchmark symlinks are forbidden');
      if (!entry.isDirectory()) continue;
      const dir = path.posix.join(relative, entry.name);
      if (entry.name === 'new') {
        const candidate = readJson(root, `${dir}/estimates.json`);
        const baseline = readJson(root, `${relative ? relative + '/' : ''}base/estimates.json`);
        if (candidate) rows.push({ name: relative, baseNs: baseline?.mean?.point_estimate ?? null,
          candidateNs: candidate?.mean?.point_estimate ?? null });
        if (rows.length > 100) throw new Error('Too many benchmark rows');
      } else if (!['base', 'change', 'report'].includes(entry.name)) walk(dir, depth + 1);
    }
  }
  walk('', 0);
  return rows;
}
function prepare(root) {
  return normalizeReports({
    coverageBase: readJson(path.join(root, 'coverage'), 'ci-base/coverage.json'),
    coverageCandidate: readJson(path.join(root, 'coverage'), 'coverage.json'),
    evalBase: readJson(path.join(root, 'eval'), 'baseline/evals.json'),
    evalCandidate: readJson(path.join(root, 'eval'), 'evals.json'),
    benchmark: benchmarkRows(path.join(root, 'benchmark')),
  });
}
if (require.main === module) {
  const data = JSON.stringify(prepare(process.argv[2]));
  if (Buffer.byteLength(data) > 128 * 1024) throw new Error('Normalized report too large');
  // Single-line JSON prevents workflow-command/output injection.
  fs.appendFileSync(process.env.GITHUB_OUTPUT, `data=${data}\n`);
}
module.exports = { readJson, benchmarkRows, prepare };
