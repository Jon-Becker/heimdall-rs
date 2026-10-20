const assert = require('node:assert/strict');
const { test } = require('node:test');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { readJson, benchmarkRows, prepare } = require('../scripts/prepare-ci-reports.cjs');
function fixture(t) { const root = fs.mkdtempSync(path.join(os.tmpdir(), 'ci-report-')); t.after(() => fs.rmSync(root, {recursive:true,force:true})); return root; }
function write(root, file, value) { fs.mkdirSync(path.dirname(path.join(root,file)),{recursive:true});fs.writeFileSync(path.join(root,file),JSON.stringify(value)); }
test('missing artifacts yield no invented measurements', t => { const root=fixture(t);const data=prepare(root);assert.deepEqual(data.coverage,{base:null,candidate:null});assert.deepEqual(data.eval.rows,[]); });
test('reads exact uploaded coverage, eval and Criterion layout', t => {const root=fixture(t);
write(root,'coverage/ci-base/coverage.json',{data:[{totals:{lines:{percent:50}}}]});write(root,'coverage/coverage.json',{data:[{totals:{lines:{percent:55}}}]});
write(root,'eval/baseline/evals.json',{test:{cfg:70,decompilation:60}});write(root,'eval/evals.json',{test:{cfg:80,decompilation:65}});
write(root,'benchmark/group/case/base/estimates.json',{mean:{point_estimate:100}});write(root,'benchmark/group/case/new/estimates.json',{mean:{point_estimate:90}});
const data=prepare(root);assert.deepEqual(data.coverage,{base:50,candidate:55});assert.equal(data.eval.rows[0].baseCfg,70);assert.deepEqual(data.benchmark.rows,[{name:'group/case',baseNs:100,candidateNs:90}]);});
test('rejects symlink files and roots',t=>{const root=fixture(t);write(root,'target.json',{});fs.symlinkSync(path.join(root,'target.json'),path.join(root,'linked.json'));assert.throws(()=>readJson(root,'linked.json'),/symlink/);fs.symlinkSync(root,path.join(root,'linked-dir'));assert.throws(()=>readJson(path.join(root,'linked-dir'),'target.json'),/symlink/);});
test('rejects path traversal and oversized files',t=>{const root=fixture(t);assert.throws(()=>readJson(root,'../secret'));fs.writeFileSync(path.join(root,'big'),Buffer.alloc(8*1024*1024+1));assert.throws(()=>readJson(root,'big'),/bounded/);});
test('rejects benchmark symlinks and unbounded depth',t=>{const root=fixture(t);fs.symlinkSync(root,path.join(root,'link'));assert.throws(()=>benchmarkRows(root),/symlink/);fs.unlinkSync(path.join(root,'link'));fs.mkdirSync(path.join(root,...Array(14).fill('d')),{recursive:true});assert.throws(()=>benchmarkRows(root),/nesting/);});
