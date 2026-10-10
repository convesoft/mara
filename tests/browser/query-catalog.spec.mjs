import { test, expect } from '@playwright/test';
import { mkdtemp, mkdir, readFile, writeFile, rm } from 'node:fs/promises';
import { spawn, execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { once } from 'node:events';
import path from 'node:path';
const exec = promisify(execFile);

// @mara checks REQ-QUERY-ENGINE-SURFACES
test('all twelve catalog witnesses agree in installed Chromium HTTP and bundled worker/WASM', async ({ page, baseURL }, info) => {
  test.setTimeout(180000);
  test.skip(!process.env.MARA_QUERY_CATALOG_REPORT, 'Catalog proof requires executed native inputs/outcomes');
  const cases = JSON.parse(await readFile(process.env.MARA_QUERY_CATALOG_REPORT, 'utf8'));
  expect([...new Set(cases.map(c=>c.witness))].sort((a,b)=>a-b)).toEqual(Array.from({length:12},(_,i)=>i+1));
  const fixture = await mkdtemp(path.join(process.env.TMPDIR, 'catalog-browser-'));
  const runtime = await mkdtemp(path.join(process.env.MARA_TEST_RUNTIME_DIR, 'q-'));
  const roots = [];
  for (const [i,c] of cases.entries()) {
    const root = path.join(fixture, 'case-' + i); roots.push(root);
    for (const f of c.snapshot.files) {
      const target = path.join(root, f.path);
      await mkdir(path.dirname(target), {recursive:true}); await writeFile(target, f.content);
    }
  }
  const env = Object.fromEntries(Object.entries(process.env).filter(([k])=>!k.startsWith('GIT_') && !k.startsWith('MARA_')));
  Object.assign(env, { GIT_CONFIG_NOSYSTEM:'1', GIT_CONFIG_GLOBAL:'/dev/null', GIT_CEILING_DIRECTORIES:fixture, XDG_CONFIG_HOME:path.join(fixture,'config') });
  const cli = args => exec(process.env.MARA_CLI, ['--format','json','--daemon-directory',runtime,...args], {cwd:fixture,env,timeout:120000});
  const daemon = spawn(process.env.MARA_CLI, ['--execution-mode','direct','--daemon-directory',runtime,'daemon','run',
    '--http-bind','127.0.0.1:0','--http-origin',baseURL,...roots.flatMap(r=>['--http-project',r])], {cwd:fixture,env,stdio:'ignore'});
  const exited = once(daemon,'exit');
  const requests = []; page.on('request', r=>requests.push(r.url()));
  try {
    let connection; const deadline = Date.now()+10000;
    while (!connection) {
      try { connection = JSON.parse((await cli(['daemon','connection-info'])).stdout); }
      catch { if (daemon.exitCode!==null || Date.now()>deadline) throw new Error('Catalog daemon did not become ready'); await new Promise(r=>setTimeout(r,20)); }
    }
    await page.goto('/');
    const actual = await page.evaluate(async ({cases,roots,connection}) => {
      const { createHttpClient } = await import('./http.mjs');
      const { createWasmClient } = await import('./browser.mjs');
      const outputs=[];
      for (const [i,c] of cases.entries()) {
        const wasm = await createWasmClient({snapshot:c.snapshot});
        const http = await createHttpClient({...connection,project:roots[i]});
        try {
          const method=c.operation.replace(/_([a-z])/g,(_,l)=>l.toUpperCase());
          for (const [host,client] of [['http',http],['wasm',wasm]]) {
            let outcome;
            try { outcome={result:await client[method](c.params)}; }
            catch (error) {outcome={error:error.envelope.error};}
            if (outcome.result) delete outcome.result.project;
            outputs.push({name:c.name,witness:c.witness,host,outcome});
          }
        } finally {await wasm.close();await http.close();}
      }
      return outputs;
    }, {cases,roots,connection:{endpoint:connection.endpoint,token:connection.token}});
    await info.attach('actual-twelve-witness-host-results',{body:Buffer.from(JSON.stringify({actual,requests},null,2)),contentType:'application/json'});
    for (const [i,c] of cases.entries()) {
      expect(actual[i*2].outcome,c.name+' HTTP').toEqual(c.outcome);
      expect(actual[i*2+1].outcome,c.name+' WASM').toEqual(c.outcome);
    }
    // Only explicit daemon RPC and same-origin installed assets were accessed.
    const allowed=new Set([new URL(page.url()).origin,new URL(connection.endpoint).origin]);
    expect(requests.every(url=>allowed.has(new URL(url).origin))).toBe(true);
  } finally {
    try {await cli(['daemon','stop']);} finally {
      if (daemon.exitCode===null) daemon.kill('SIGTERM');
      await exited; await rm(fixture,{recursive:true,force:true}); await rm(runtime,{recursive:true,force:true});
    }
  }
});
