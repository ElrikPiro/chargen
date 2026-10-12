// Independent acceptance for E4: known demographic facts, read-only presentation and local workflows.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { resolve } from 'node:path';
import { createHash } from 'node:crypto';
import { setTimeout as delay } from 'node:timers/promises';
const binary = resolve(process.argv[2] || 'target/release/chargen-app');
const scratch = await mkdtemp(resolve('target','e4-acceptance-'));
const database = resolve(scratch,'worlds.sqlite3');
const scenario = JSON.parse(await readFile('scenarios/demos/b0-family.json','utf8'));
scenario.world_id=88001;
scenario.settlements[0].name='Roble | [enlace](javascript:alert(1)) <script>peligro</script> `texto`';
scenario.policies.mortality={default_probability_ppm:0,age_bands:[{minimum_age:60,maximum_age:null,probability_ppm:1000000}]};
scenario.policies.unions.enabled=false;scenario.policies.births.probability_ppm=1000000;scenario.policies.births.minimum_years_between_births=1;
scenario.initial_people.push({...structuredClone(scenario.initial_people[2]),id:4,logical_id:'2004',birth_year:-80,household_id:null});
scenario.initial_family_links.push({id:3,logical_id:'5003',parent:1,child:3,kind:'caregiver',validity:{start:-3,end_exclusive:null}});
scenario.genetics={loci:[{id:'test::pigment',variants:['A','b'],founder_distribution:[{variant:'A',weight:1},{variant:'b',weight:1}],expression:{mode:'additive',values:{A:2,b:5}}}]};
scenario.initial_people.forEach(p=>p.genome={loci:{'test::pigment':[{variant:'A',lineage:null,inherited_from:null},{variant:'b',lineage:null,inherited_from:null}]}});
// The wire contract canonically serializes logical IDs as decimal text and absent trait metadata as null.
for(const rows of [scenario.settlements,scenario.initial_people,scenario.initial_households,scenario.initial_unions,scenario.initial_family_links])for(const row of rows)row.logical_id=String(row.logical_id);
scenario.trait_catalog.forEach(trait=>{trait.unit??=null;trait.description??=null;});
let child,base,serverErrors='';
async function start(){
 child=spawn(binary,['serve','--database',database,'--bind','127.0.0.1:0','--no-open'],{cwd:scratch,env:{...process.env,PATH:scratch},stdio:['ignore','pipe','pipe']});
 let output='';serverErrors='';child.stderr.on('data',x=>serverErrors+=x);
 base=await new Promise((ok,no)=>{const timer=setTimeout(()=>no(new Error(serverErrors||'startup timeout')),10000);child.stdout.on('data',x=>{output+=x;const match=output.match(/Chargen listening on (http:\/\/127\.0\.0\.1:\d+\/)/);if(match){clearTimeout(timer);ok(match[1]);}});child.once('error',no);child.once('exit',code=>{clearTimeout(timer);no(new Error(`server exited ${code}: ${serverErrors}`));});});
}
async function stop(){if(!child||child.exitCode!==null)return;const done=new Promise(ok=>child.once('exit',ok));child.kill();await Promise.race([done,delay(5000)]);if(child.exitCode===null)child.kill('SIGKILL');await done;child=undefined;}
async function call(path,body,expected=200){const response=await fetch(new URL(path,base),{method:body===undefined?'GET':'POST',headers:{'content-type':'application/json'},body:body===undefined?undefined:JSON.stringify(body)});const raw=await response.text();let data=null;if(response.status!==204){try{data=JSON.parse(raw);}catch{throw new Error(`${path}: HTTP ${response.status} ${raw}`);}}assert.equal(response.status,expected,`${path}: ${JSON.stringify(data)} ${response.status>=500?serverErrors:''}`);return data;}
async function textResponse(path){const response=await fetch(new URL(path,base));assert.equal(response.status,200,path);assert.match(response.headers.get('content-type'),/text\/markdown/);return response.text();}
async function run(branch,year,prefix='api'){
 const job=await call(`${prefix}/worlds/88001/branches/${branch}/runs`,{through_year:year});
 for(let n=0;n<500;n++){const status=await call(`${prefix}/runs/${job.job_id}`);if(status.status==='completed'){assert.equal(status.current_year,year);return status;}assert.notEqual(status.status,'failed',JSON.stringify(status));await delay(20);}throw new Error('run timeout');
}
const endpoint=(branch,kind,year,prefix='api')=>`${prefix}/worlds/88001/branches/${branch}/${kind}?year=${year}&limit=100`;
const view=(kind,year,branch='main',prefix='api')=>call(endpoint(branch,kind,year,prefix));
const withoutBranch=value=>({...value,branch_id:undefined});
try {
 await start();
 for(const [route,file] of [['','index.html'],['app.js','app.js'],['styles.css','styles.css'],['scenario-editor.js','scenario-editor.js'],['economic-amounts.js','economic-amounts.js']]){
  const resource=await fetch(new URL(route,base));assert.equal(resource.status,200,route);const digest=text=>createHash('sha256').update(text).digest('hex');assert.equal(digest(await resource.text()),digest(await readFile(`web/${file}`,'utf8')),`embedded resource must match reviewed source: ${file}`);
 }
 assert.equal((await call('api/scenarios/validate',{scenario})).valid,true);await call('api/worlds',{scenario});
 const schema=await call('api/scenarios/schema?version=1');assert.equal(schema.version,1);assert.ok(schema.schema.$defs.Scenario.properties);await call('api/scenarios/schema?version=4',undefined,400);
 const initialRun=await run('main',0);
 const progress=await fetch(new URL(`api/runs/${initialRun.job_id}/progress`,base),{signal:AbortSignal.timeout(8000)});assert.equal(progress.status,200);assert.match(progress.headers.get('content-type'),/text\/event-stream/);const streamed=await progress.text();assert.match(streamed,/event: ?progress/);assert.ok(streamed.includes('completed'));
 assert.equal((await view('census',0)).total_population,4);
 const people0=await view('people',0),events0=await view('events',0);
 const originalScenario=await call('api/worlds/88001/branches/main/scenario?year=0');assert.deepEqual(originalScenario.scenario,scenario);
 await call('api/worlds/88001/branches',{parent_branch_id:'main',branch_id:'untouched',cut_year:0,scenario});
 const firstDossier=await view('people/1/dossier',0);
 assert.equal(firstDossier.status.status,'alive');assert.equal(firstDossier.birth_year,-25);assert.equal(firstDossier.death_year,null);assert.equal(firstDossier.age_at_close,25);
 assert.equal(firstDossier.record.person.traits['cultura::cooperacion'],55);assert.ok(firstDossier.presentation.name);assert.match(firstDossier.fact_fingerprint,/^fnv1a64:[a-f0-9]{16}$/);
 assert.equal(firstDossier.family_links_total,3);assert.equal(firstDossier.family_links_truncated,false);assert.equal(firstDossier.family_links.length,3);
 assert.deepEqual([...new Set(firstDossier.family_links.map(x=>x.relative.person_id))].sort((a,b)=>a-b),[3,5]);assert.ok(firstDossier.timeline.every(x=>x.year<=0));assert.equal(firstDossier.genome[0].expressed_value,7);assert.equal(firstDossier.settlement_name,scenario.settlements[0].name);assert.equal(firstDossier.unions_total,1);assert.equal(firstDossier.unions[0].participants_total,2);assert.deepEqual(firstDossier.unions[0].participants.map(x=>x.person_id),[1,2]);
 const unionMembers=await call(endpoint('main','unions/1/members',0).replace('limit=100','limit=1'));assert.equal(unionMembers.total,2);assert.equal(unionMembers.participants.length,1);
 const childDossier=await view('people/5/dossier',0);const alleles=childDossier.record.person.genome.loci['test::pigment'];assert.deepEqual(alleles.map(x=>x.inherited_from).sort(),['2001','2002']);assert.equal(childDossier.genome[0].expressed_value,alleles.reduce((sum,x)=>sum+(x.variant==='A'?2:5),0));
 const deceased=await view('people/4/dossier',0);assert.equal(deceased.status.status,'deceased');assert.equal(deceased.death_year,0);assert.equal(deceased.age_at_death,80);assert.equal(deceased.age_at_close,null);
 const notBorn=await view('people/6/dossier',0),unknown=await view('people/999/dossier',0);assert.equal(notBorn.status.status,'not_known_at_year');assert.equal(unknown.status.status,'not_known_at_year');assert.equal(notBorn.record,null);assert.equal(notBorn.presentation,null);assert.equal(notBorn.birth_year,null);
 const page0=await call(endpoint('main','people/1/tree',0)+'&direction=descendants&depth=1&offset=0');assert.equal(page0.total,3);assert.equal(page0.relations.length,3);assert.ok(page0.relations.every(x=>x.distance===1));
 const pages=[];for(let offset=0;offset<3;offset++)pages.push((await call(endpoint('main','people/1/tree',0).replace('limit=100','limit=1')+`&direction=descendants&depth=1&offset=${offset}`)).relations[0]);assert.deepEqual(pages,page0.relations);
 const biological=await call(endpoint('main','people/1/tree',0)+'&direction=descendants&depth=1&kinds=biological_parent');assert.equal(biological.total,2);
 const care=await call(endpoint('main','people/1/tree',0)+'&direction=descendants&depth=1&kinds=caregiver');assert.equal(care.total,1);assert.equal(care.relations[0].relative.person.id,3);
 await call(endpoint('main','people/1/tree',0).replace('limit=100','limit=101')+'&direction=descendants',undefined,400);
 const household=await view('households/1',0);assert.equal(household.total,4);assert.deepEqual(household.members.map(x=>x.person_id),[1,2,3,5]);assert.equal(household.active_at_close,true);
 const place=await view('settlements/1/dossier',0);assert.equal(place.settlement.population,4);assert.equal(place.people.length,4);assert.equal(place.household_count,1);
 const md0=await textResponse(endpoint('main','people/1/markdown',0));assert.ok(md0.includes(firstDossier.presentation.name));assert.ok(!md0.includes('<script>'));assert.ok(!md0.includes('[enlace](javascript:'));
 const mdSettlement=await textResponse(endpoint('main','settlements/1/markdown',0));assert.ok(!mdSettlement.includes('<script>'));assert.ok(!mdSettlement.includes('[enlace](javascript:'));assert.ok(mdSettlement.includes('\\|'));
 const mdHousehold=await textResponse(endpoint('main','households/1/markdown',0));assert.ok(mdHousehold.includes('Hogar'));

 const archive=await call('api/worlds/88001/export',{});
 const downloaded=await fetch(new URL(archive.download_url,base));assert.equal(downloaded.status,200);const archiveBytes=await downloaded.arrayBuffer();assert.ok(archiveBytes.byteLength>1024);
 const restored=await call('api/restore',{export_id:archive.export_id});assert.equal(restored.restart_required,false);assert.ok(restored.api_base.startsWith('/api/restores/'));
 const restorePrefix=restored.api_base.replace(/^\//,'');assert.deepEqual((await call(`${restorePrefix}/worlds`)).map(x=>x.world_id),['88001']);
 assert.deepEqual(await view('people',0,'main',restorePrefix),people0);
 await run('main',1,restorePrefix);assert.equal((await view('census',1,'main',restorePrefix)).total_population,5);
 assert.equal((await call('api/worlds/88001/branches'))[0].head_year,0);
 const importedResponse=await fetch(new URL('api/import/archive',base),{method:'POST',headers:{'content-type':'application/octet-stream'},body:archiveBytes});assert.equal(importedResponse.status,200);const imported=await importedResponse.json();assert.ok(imported.api_base.startsWith('/api/restores/'));
 const corrupt=await fetch(new URL('api/import/archive',base),{method:'POST',headers:{'content-type':'application/octet-stream'},body:new TextEncoder().encode('not a SQLite archive')});assert.equal(corrupt.status,400);
 await run('main',2);await run('untouched',2);
 assert.equal((await view('census',2)).total_population,6);
 assert.deepEqual(withoutBranch(await view('people',2)),withoutBranch(await view('people',2,'untouched')));
 assert.deepEqual((await view('events',2)).events,(await view('events',2,'untouched')).events);
 assert.deepEqual(await view('people',0),people0);assert.deepEqual(await view('events',0),events0);assert.deepEqual(await view('people/1/dossier',0),firstDossier);assert.equal(await textResponse(endpoint('main','people/1/markdown',0)),md0);assert.equal((await view('people/6/dossier',1)).birth_year,1);
 const revised=structuredClone(scenario);revised.policies.births.probability_ppm=0;
 await call('api/worlds/88001/branches',{parent_branch_id:'main',branch_id:'no-births',cut_year:0,scenario:revised});await run('no-births',2);
 assert.equal((await view('census',2,'no-births')).total_population,4);
 const comparison=await call('api/worlds/88001/compare?left_branch_id=main&left_year=2&right_branch_id=no-births&right_year=2&limit=1&offset=0');assert.equal(comparison.items.length,1);assert.equal(comparison.items[0].left.settlement.population,6);assert.equal(comparison.items[0].right.settlement.population,4);assert.equal(comparison.left.year,2);assert.equal(comparison.right.branch_id,'no-births');
 const concurrent=structuredClone(scenario);concurrent.world_id=88002;concurrent.policies.births.enabled=false;concurrent.policies.mortality={default_probability_ppm:0,age_bands:[]};await call('api/worlds',{scenario:concurrent});
 async function runStable(branch,year){const job=await call(`api/worlds/88002/branches/${branch}/runs`,{through_year:year});for(let n=0;n<500;n++){const status=await call(`api/runs/${job.job_id}`);if(status.status==='completed')return status;assert.notEqual(status.status,'failed',JSON.stringify(status));await delay(20);}throw new Error('stable run timeout');}
 await runStable('main',0);await call('api/worlds/88002/branches',{parent_branch_id:'main',branch_id:'control',cut_year:0,scenario:concurrent});
 const stablePath='api/worlds/88002/branches/main/people/1/dossier?year=0';const stableCard=await call(stablePath);
 const long=await call('api/worlds/88002/branches/main/runs',{through_year:10000});let observedRunning=false;
 for(let n=0;n<5;n++){const status=await call(`api/runs/${long.job_id}`);if(status.status==='running')observedRunning=true;assert.deepEqual(await call(stablePath),stableCard);assert.equal((await fetch(new URL('api/worlds/88002/branches/main/people/1/markdown?year=0',base))).status,200);if(status.status==='completed')break;}
 assert.equal(observedRunning,true,'observe read-only dossier/Markdown requests during an active simulation');await call(`api/runs/${long.job_id}/cancel`,{});let terminal;
 for(let n=0;n<500;n++){terminal=await call(`api/runs/${long.job_id}`);if(['cancelled','completed'].includes(terminal.status))break;assert.notEqual(terminal.status,'failed');await delay(20);}assert.ok(['cancelled','completed'].includes(terminal.status));
 await runStable('control',terminal.current_year);const stateA=await call(`api/worlds/88002/branches/main/people?year=${terminal.current_year}`),stateB=await call(`api/worlds/88002/branches/control/people?year=${terminal.current_year}`);assert.deepEqual(withoutBranch(stateA),withoutBranch(stateB));
 assert.deepEqual((await call(`api/worlds/88002/branches/main/events?year=${terminal.current_year}`)).events,(await call(`api/worlds/88002/branches/control/events?year=${terminal.current_year}`)).events);
 await stop();await start();
 const copies=await call('api/restores');assert.ok(copies.restores.length>=2);
 assert.equal((await view('census',1,'main',restorePrefix)).total_population,5);
 assert.equal((await view('census',2)).total_population,6);
 console.log('E4 black-box acceptance passed: independently calculated history, read-only dossiers, bounded genealogy, escaped Markdown, temporal branches/comparison, durable restored catalogs, validated binary import, isolation and restart.');
}finally{await stop();await rm(scratch,{recursive:true,force:true});}
