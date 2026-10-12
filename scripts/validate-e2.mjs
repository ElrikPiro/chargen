// Black-box acceptance: expected facts are hand-calculated, independent of the Rust algorithms.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { resolve } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
const binary = resolve(process.argv[2] || 'target/release/chargen-app');
const scratch = await mkdtemp(resolve('target', 'e2-acceptance-'));
const database = resolve(scratch, 'worlds.sqlite3');
const scenario = JSON.parse(await readFile('scenarios/demos/e2-territory-culture.json', 'utf8'));
scenario.world_id = 99001;
const key='cultura::cooperacion';
[20,40,80].forEach((value,i)=>scenario.initial_people[i].traits[key]=value);
scenario.initial_people.push({...structuredClone(scenario.initial_people[0]),id:4,logical_id:'2004',birth_year:-30,household_id:2,traits:{[key]:100}});
scenario.initial_households.push({...structuredClone(scenario.initial_households[0]),id:2,logical_id:'3002',members:[4]});
scenario.initial_unions[0].participants=[1,2,4];
scenario.policies.births.enabled=true;scenario.policies.births.probability_ppm=1000000;
scenario.territory.migration.unit='household';
scenario.territory.settlement_capacities={'2':6};
scenario.territory.routes[0].capacity_per_year=4;
scenario.territory.routes[0].open_intervals=[{start:1,end_exclusive:2}];
scenario.territory.professions=[{id:'farmer',name:'Agricultor'}];
scenario.territory.profession_options_by_settlement={'1':['farmer'],'2':[]};
scenario.initial_people[0].profession='farmer';
const institution=structuredClone(scenario.culture.institutions[0]);
institution.affiliation.minimum_age=18;institution.affiliation.minimum_affinity_ppm=1000000;
scenario.culture.institutions=[{...structuredClone(institution),id:'assembly',allowed_settlements:[1],values:{[key]:100}},{...structuredClone(institution),id:'faith',kind:'religion',allowed_settlements:[2],values:{[key]:40}}];
Object.assign(scenario.culture.organizations,{minimum_founders:3,maximum_members:3,dissolve_below_members:2});
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
const view=async(branch,kind,year)=>{const data=await call(`api/worlds/99001/branches/${branch}/${kind}?year=${year}`);if(data.items){const field=({culture:'settlements',migration:'settlements',organizations:'organizations',routes:'routes'})[kind];if(field)data[field]=data.items;}return data;};
const cultureRow=(data,id)=>data.settlements.find(x=>x.settlement_id===id).traits.find(x=>x.key===key);
try {
 await start();
 assert.equal((await call('api/scenarios/validate',{scenario})).valid,true);
 await call('api/worlds',{scenario});
 await run('main',0);
 assert.equal((await census('main',0)).total_population,6);
 assert.equal((await person('main',5,0)).person.person.traits[key],40);
 assert.equal((await person('main',6,0)).person.person.traits[key],66);
 const c0=await view('main','culture',0);
 assert.equal(cultureRow(c0,1).weighted_sum,346);assert.equal(cultureRow(c0,1).current_value,58);
 assert.equal(cultureRow(c0,2).current_value,null);
 const o0=await view('main','organizations',0);
 assert.equal(o0.organizations.length,1);assert.deepEqual(o0.organizations[0].organization.founders,[1,2,4]);
 assert.equal(o0.organizations[0].organization.dissolved_year,null);
 const orgValue=o0.organizations[0].organization.culture_values.find(x=>x.key===key);
 assert.equal(orgValue.weighted_sum,160);assert.equal(orgValue.current_value,53);
 assert.equal((await view('main','people/4/affiliations',0)).institutions[0].institution_id,'assembly');
 const revised=structuredClone(scenario);revised.territory.scheduled_displacements=[];revised.territory.route_revision=2;
 await call('api/worlds/99001/branches',{parent_branch_id:'main',branch_id:'stay',cut_year:0,scenario:revised});
 assert.equal((await view('stay','routes',0)).route_revision,1);
 await run('main',1);await run('stay',1);
 assert.equal((await view('stay','routes',1)).route_revision,2);
 assert.deepEqual((await census('main',1)).settlements.map(x=>x.population),[3,5]);
 assert.deepEqual((await census('stay',1)).settlements.map(x=>x.population),[8,0]);
 assert.equal((await person('main',7,1)).person.person.traits[key],35);
 // Active-union contributors retain their identity across residence changes.
 assert.equal((await person('main',8,1)).person.person.traits[key],74);
 const c1=await view('main','culture',1);
 assert.equal(cultureRow(c1,1).current_value,80);assert.equal(cultureRow(c1,2).current_value,43);
 for(const [id,value] of [[1,20],[2,40],[3,80],[4,100]])assert.equal((await person('main',id,1)).person.person.traits[key],value);
 const residence=await view('main','people/1/residence',1);
 assert.deepEqual(residence.residences.map(x=>x.settlement_id),[1,2]);
 assert.equal(residence.professions[0].validity.end_exclusive,1);
 const historical=await view('main','people/1/residence',0);
 assert.equal(historical.residences.length,1);assert.equal(historical.residences[0].validity.end_exclusive,null);
 assert.equal(historical.professions[0].validity.end_exclusive,null);
 const faith=await view('main','people/2/affiliations',1);
 assert.ok(faith.institutions.some(x=>x.institution_id==='faith'));
 assert.equal((await view('main','people/2/affiliations',0)).institutions.length,0);
 const o1=await view('main','organizations',1);
 assert.equal(o1.organizations[0].organization.dissolved_year,1);
 assert.deepEqual(o1.organizations[0].organization.founders,[1,2,4]);
 assert.deepEqual(await view('main','organizations',0),o0);
 assert.deepEqual(await view('main','culture',0),c0);
 assert.equal((await view('stay','organizations',1)).organizations[0].organization.dissolved_year,null);
 const balances=await view('main','migration',1);
 for(const row of balances.settlements)assert.equal(row.before+row.births-row.deaths+row.incoming-row.outgoing,row.after);
 assert.equal((await view('main','routes',1)).routes.length,1);
 await run('main',2);assert.equal((await view('main','routes',2)).routes.length,0);
 assert.deepEqual(await view('main','culture',0),c0);
 const archive=await call('api/worlds/99001/export',{});await call('api/restore',{export_id:archive.export_id});
 await stop();await start();
 assert.deepEqual(await view('main','culture',1),c1);
 assert.deepEqual((await census('stay',1)).settlements.map(x=>x.population),[8,0]);
 console.log('E2 black-box acceptance passed: independently calculated birth batch 40/66, empty culture, household movement, profession and residence history, immutable traits, temporal affiliations, organization founders/dissolution, balances, route closure, edited future branch, export/restore and restart.');
} finally {await stop();await rm(scratch,{recursive:true,force:true});}
