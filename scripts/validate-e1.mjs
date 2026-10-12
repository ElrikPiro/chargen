// Black-box acceptance: expected facts are hand-calculated, independent of the Rust algorithms.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { resolve } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
const binary = resolve(process.argv[2] || 'target/release/chargen-app');
const scratch = await mkdtemp(resolve('target', 'e1-acceptance-'));
const database = resolve(scratch, 'worlds.sqlite3');
const scenario = JSON.parse(await readFile('scenarios/demos/b0-family.json', 'utf8'));
scenario.world_id = 99001;
scenario.policies.mortality = {default_probability_ppm: 0, age_bands: [{minimum_age: 60, maximum_age: null, probability_ppm: 1000000}]};
scenario.policies.unions.enabled = false;
scenario.policies.births.probability_ppm = 1000000;
scenario.policies.births.minimum_years_between_births = 1;
scenario.initial_people.push({...structuredClone(scenario.initial_people[2]), id: 4, logical_id: '2004', birth_year: -80, household_id: null});
let child, base;
async function start() {
  child = spawn(binary, ['serve','--database',database,'--bind','127.0.0.1:0','--no-open'], {cwd:scratch,env:{...process.env,PATH:scratch},stdio:['ignore','pipe','pipe']});
  let output='',errors=''; child.stderr.on('data',x=>errors+=x);
  base = await new Promise((ok,no)=>{const timer=setTimeout(()=>no(new Error(errors || 'startup timeout')),10000);child.stdout.on('data',x=>{output+=x;const m=output.match(/Chargen listening on (http:\/\/127\.0\.0\.1:\d+\/)/);if(m){clearTimeout(timer);ok(m[1]);}});child.once('error',no);child.once('exit',code=>{clearTimeout(timer);no(new Error(`server exited ${code}: ${errors}`));});});
}
async function stop() {if(!child || child.exitCode!==null)return;const done=new Promise(ok=>child.once('exit',ok));child.kill();await Promise.race([done,delay(5000)]);if(child.exitCode===null)child.kill('SIGKILL');await done;child=undefined;}
async function call(path, body, expected=200) {
 const r=await fetch(new URL(path,base),{method:body===undefined?'GET':'POST',headers:{'content-type':'application/json'},body:body===undefined?undefined:JSON.stringify(body)});
 const data=r.status===204?null:await r.json();assert.equal(r.status,expected,`${path}: ${JSON.stringify(data)}`);return data;
}
async function run(branch, year, world = 99001) {
 const job=await call(`api/worlds/${world}/branches/${branch}/runs`,{through_year:year});
 for(let n=0;n<200;n++){const result=await call(`api/runs/${job.job_id}`);if(result.status==='completed'){assert.equal(result.current_year,year);return result;}assert.notEqual(result.status,'failed',JSON.stringify(result));await delay(20);}
 throw new Error('run timeout');
}
const census=(branch,year)=>call(`api/worlds/99001/branches/${branch}/census?year=${year}`);
const person=(branch,id,year)=>call(`api/worlds/99001/branches/${branch}/people/${id}?year=${year}`);
try {
 await start();
 assert.equal((await call('api/scenarios/validate',{scenario})).valid,true);
 await call('api/worlds',{scenario});
 await run('main',2);
 for(const [year,count]of[[0,4],[1,5],[2,6]])assert.equal((await census('main',year)).total_population,count);
 const past=await person('main',1,0);assert.equal(past.person.person.birth_year,-25);assert.equal(past.person.person.death_year,null);
 const family=await call('api/worlds/99001/branches/main/people/1/genealogy?year=0&depth=2');
 assert.deepEqual([...new Set(family.descendants.map(x=>x.relative.person.id))].sort((a,b)=>a-b),[3,5]);
 await call('api/worlds/99001/branches/main/people/6?year=0',undefined,404);
 const edited=structuredClone(scenario);edited.policies.births.probability_ppm=0;
 await call('api/worlds/99001/branches',{parent_branch_id:'main',branch_id:'no-births',cut_year:0,scenario:edited});
 await run('no-births',2);
 assert.equal((await census('no-births',2)).total_population,4);
 assert.equal((await census('main',2)).total_population,6);
 await call('api/worlds/99001/branches/no-births/people/6?year=2',undefined,404);
 const incompatible=structuredClone(scenario);incompatible.initial_people[0].birth_year=-30;
 const rejected=await fetch(new URL('api/worlds/99001/branches',base),{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({parent_branch_id:'main',branch_id:'invalid-past',cut_year:0,scenario:incompatible})});
 assert.ok(rejected.status>=400 && rejected.status<500);
 assert.ok(!(await call('api/worlds/99001/branches')).some(x=>x.branch_id==='invalid-past'));
 await call('api/worlds/99001/branches',{parent_branch_id:'main',branch_id:'edited-initial',cut_year:null,scenario:incompatible});
 await run('edited-initial',0);
 assert.equal((await person('edited-initial',1,0)).person.person.birth_year,-30);
 assert.equal((await person('main',1,0)).person.person.birth_year,-25);
 const unknown=structuredClone(scenario);unknown.policies.births.unsupported_magic=true;
 await call('api/scenarios/validate',{scenario:unknown},400);
 const pausable=structuredClone(scenario);pausable.world_id=99003;await call('api/worlds',{scenario:pausable});
 const longJob=await call('api/worlds/99003/branches/main/runs',{through_year:9999});
 await call(`api/runs/${longJob.job_id}/pause`,{});
 let paused;
 for(let n=0;n<200;n++){paused=await call(`api/runs/${longJob.job_id}`);if(paused.status==='paused')break;assert.ok(!['failed','completed','cancelled'].includes(paused.status),JSON.stringify(paused));await delay(20);}
 assert.equal(paused.status,'paused');
 const durablePause=(await call('api/worlds/99003/branches')).find(x=>x.branch_id==='main').head_year;
 assert.equal(durablePause,paused.current_year);
 await call(`api/runs/${longJob.job_id}/resume`,{});
 await call(`api/runs/${longJob.job_id}/cancel`,{});
 let cancelled;
 for(let n=0;n<200;n++){cancelled=await call(`api/runs/${longJob.job_id}`);if(cancelled.status==='cancelled')break;assert.notEqual(cancelled.status,'failed');await delay(20);}
 assert.equal(cancelled.status,'cancelled');
 const durableCancel=(await call('api/worlds/99003/branches')).find(x=>x.branch_id==='main').head_year;
 assert.equal(durableCancel,cancelled.current_year);
 await run('main',(durableCancel ?? -1)+2,99003);
 const second=structuredClone(scenario);second.world_id=99002;await call('api/worlds',{scenario:second});
 const archive=await call('api/worlds/99001/export',{});
 assert.deepEqual(archive.manifest.world_ids,['99001']);
 const restored=await call('api/restore',{export_id:archive.export_id});assert.deepEqual(restored.worlds,['99001']);
 await stop();await start();
 assert.equal((await census('main',2)).total_population,6);
 assert.equal((await census('no-births',2)).total_population,4);
 assert.equal((await person('main',1,0)).person.person.birth_year,-25);
 console.log('E1 black-box acceptance passed: exact births/deaths/census, historical genealogy, policy and initial branches, prefix rejection, pause/resume/cancel and subsequent resume, isolated export/restore, restart.');
} finally {await stop();await rm(scratch,{recursive:true,force:true});}
