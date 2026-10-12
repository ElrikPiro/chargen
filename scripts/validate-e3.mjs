// Black-box acceptance: expected facts are hand-calculated, independent of the Rust algorithms.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { resolve } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
const binary = resolve(process.argv[2] || 'target/release/chargen-app');
const scratch = await mkdtemp(resolve('target', 'e3-acceptance-'));
const database = resolve(scratch, 'worlds.sqlite3');
const scenario = JSON.parse(await readFile('scenarios/demos/e2-territory-culture.json','utf8'));
scenario.world_id=77001;scenario.schema_version=3;scenario.rules_version=3;
scenario.initial_people.forEach(p=>{p.household_id=null;p.reproductive.fertility_enabled=false;});
scenario.initial_people[0].profession='farmer';
scenario.initial_people[1].birth_year=-4;
scenario.initial_people[2].settlement_id=2;scenario.initial_people[2].birth_year=-30;
scenario.initial_households=[];scenario.initial_unions=[];scenario.initial_family_links=[];
scenario.culture.institutions=[];scenario.culture.organizations.enabled=false;
scenario.territory.settlement_capacities={};scenario.territory.scheduled_displacements=[];
scenario.territory.routes=[{logical_id:'6001',from:1,to:2,cost:1,capacity_per_year:null,open_intervals:[]}];
scenario.territory.professions=[{id:'farmer',name:'Agricultor'}];
scenario.territory.profession_options_by_settlement={'1':['farmer'],'2':[]};
scenario.economy={
 goods:{food:{name:'Alimento',unit:'ración',transport_units_per_unit:1,storable:true,annual_storage_loss_ppm:0,initial_price:'2',minimum_price:'1',maximum_price:'10'}},
 profession_policies:{farmer:{recipe:{batches_per_worker:1,maximum_batches_per_year:1,inputs_per_batch:{},outputs_per_batch:{food:'4'}}}},
 needs:{food:{quantity_per_person_per_year:'1',priority:1}},route_goods_capacities:{'6001':'1'},
 transport_money_per_route_cost_unit:'1',transport_cost_account:'treasury',
 initial_group_endowments:[1,2].map(settlement_id=>({group_id:{settlement_id,profession_id:null},endowment:{money:'10',goods:{}}})),
 initial_treasury:{money:'0',goods:{}},price_policy:{adjustment_step_ppm:500000},buyer_priority:'seeded_logical_rank',
 asset_continuity:'retain_group_assets',empty_group_disposition:'retain_dormant',
 labor_policy:{enabled:false,minimum_working_age:18,max_changes_per_settlement_per_year:0},responses:{},production_response:{enabled:false}
};
const exact=count=>({mode:'exact',data:{count,overflow:'error'}});
const shock=(id,name,priority,population_filter,effects)=>({id:String(id),name,start_year:2,duration_years:1,priority,population_filter,effects});
scenario.scheduled_events=[
 shock(9001,'Daño registrado',30,{settlement_ids:[1],profession_ids:['farmer']},[{type:'stock_destruction',data:{loss_ppm:1000000}}]),
 shock(9002,'Incapacidad temporal',20,{settlement_ids:[1],profession_ids:['farmer']},[{type:'work_incapacity',data:{selection:exact(1)}}]),
 shock(9003,'Cierre temporal',15,{},[{type:'route_closure',data:{route_ids:['6001']}}]),
 shock(9004,'Baja única',10,{settlement_ids:[2]},[{type:'deaths',data:{selection:exact(1)}},{type:'deaths',data:{selection:{mode:'probabilistic',data:{probability_ppm:1000000}}}}])
];
let child, base, serverErrors='';
async function start() {
  child = spawn(binary, ['serve','--database',database,'--bind','127.0.0.1:0','--no-open'], {cwd:scratch,env:{...process.env,PATH:scratch},stdio:['ignore','pipe','pipe']});
  let output='',errors=''; child.stderr.on('data',x=>{errors+=x;serverErrors+=x;});
  base = await new Promise((ok,no)=>{const timer=setTimeout(()=>no(new Error(errors || 'startup timeout')),10000);child.stdout.on('data',x=>{output+=x;const m=output.match(/Chargen listening on (http:\/\/127\.0\.0\.1:\d+\/)/);if(m){clearTimeout(timer);ok(m[1]);}});child.once('error',no);child.once('exit',code=>{clearTimeout(timer);no(new Error(`server exited ${code}: ${errors}`));});});
}
async function stop() {if(!child || child.exitCode!==null)return;const done=new Promise(ok=>child.once('exit',ok));child.kill();await Promise.race([done,delay(5000)]);if(child.exitCode===null)child.kill('SIGKILL');await done;child=undefined;}
async function call(path, body, expected=200) {
 const r=await fetch(new URL(path,base),{method:body===undefined?'GET':'POST',headers:{'content-type':'application/json'},body:body===undefined?undefined:JSON.stringify(body)});
 const data=r.status===204?null:await r.json();assert.equal(r.status,expected,`${path}: ${JSON.stringify(data)} ${r.status>=500?serverErrors:''}`);return data;
}
async function run(branch, year, world = 77001) {
 const job=await call(`api/worlds/${world}/branches/${branch}/runs`,{through_year:year});
 for(let n=0;n<400;n++){const result=await call(`api/runs/${job.job_id}`);if(result.status==='completed'){assert.equal(result.current_year,year);return result;}assert.notEqual(result.status,'failed',JSON.stringify(result));await delay(20);}
 throw new Error('run timeout');
}
const route=(world,branch,kind,year,extra='')=>`api/worlds/${world}/branches/${branch}/${kind}?year=${year}&limit=100${extra}`;
const view=(kind,year,branch='main',world=77001,extra='')=>call(route(world,branch,kind,year,extra));
const amount=value=>BigInt(value);
const group=(data,place,profession=null)=>data.groups.find(x=>x.group_id.settlement_id===place&&x.group_id.profession_id===profession);
const market=(data,place)=>data.settlements.find(x=>x.settlement_id===place).goods.find(x=>x.good_id==='food');
function balanced(data) {
 const m=data.money;
 assert.equal(amount(m.opening_group_money)+amount(m.opening_treasury_money)+amount(m.explicit_emission)-amount(m.explicit_retirement)-amount(m.external_transport_outflow),amount(m.closing_group_money)+amount(m.closing_treasury_money));
 for(const row of data.groups){
  assert.equal(amount(row.opening_money)+amount(row.sales_income)-amount(row.purchases_spent)+amount(row.transfers_in)-amount(row.transfers_out),amount(row.closing_money));
  for(const g of Object.values(row.goods))assert.equal(amount(g.opening_stock)+amount(g.produced)-amount(g.input_consumed)+amount(g.transfers_in)-amount(g.transfers_out)+amount(g.bought)-amount(g.sold)-amount(g.consumer_consumed)-amount(g.storage_loss)-amount(g.event_destroyed),amount(g.closing_stock));
 }
}
try {
 await start();const validation=await call('api/scenarios/validate',{scenario});assert.equal(validation.valid,true,JSON.stringify(validation));await call('api/worlds',{scenario});
 await run('main',1);
 const first=await view('markets',0),second=await view('markets',1);
 balanced(first);balanced(second);
 assert.equal(group(first,1,'farmer').goods.food.produced,'4');
 assert.equal(group(first,1,'farmer').goods.food.closing_stock,'2');
 assert.equal(group(first,1,'farmer').closing_money,'2');
 const p0=await view('markets/settlements',0);
 assert.equal(market(p0,1).opening_price,'2');assert.equal(market(p0,1).next_price,'1');assert.equal(market(p0,2).next_price,'3');
 assert.equal((await view('markets/trades',0)).trades.filter(x=>x.route_id!==null).length,0);
 assert.equal(group(second,1,'farmer').goods.food.closing_stock,'3');assert.equal(group(second,1,'farmer').closing_money,'4');
 assert.equal(group(second,1).closing_money,'7');assert.equal(group(second,2).closing_money,'8');assert.equal(second.money.closing_treasury_money,'1');
 const trades=await view('markets/trades',1);const transport=trades.trades.filter(x=>x.route_id!==null);
 assert.equal(transport.length,1);assert.equal(transport[0].quantity,'1');assert.equal(transport[0].buyer_payment,'2');assert.equal(transport[0].transport_cost,'1');assert.equal(transport[0].source_group.settlement_id,1);assert.equal(transport[0].destination_group.settlement_id,2);
 const originalEvents=await view('events',1);assert.ok(!originalEvents.events.some(e=>e.kind.type==='scheduled_event_occurred'));
 const without=structuredClone(scenario);without.scheduled_events=[];
 await call('api/worlds/77001/branches',{parent_branch_id:'main',branch_id:'no-shocks',cut_year:1,scenario:without});
 const higherProduction=structuredClone(without);higherProduction.economy.profession_policies.farmer.recipe.outputs_per_batch.food='6';
 await call('api/worlds/77001/branches',{parent_branch_id:'main',branch_id:'recipe-six',cut_year:1,scenario:higherProduction});
 const changedEndowment=structuredClone(without);changedEndowment.economy.initial_treasury.money='999';
 await call('api/worlds/77001/branches',{parent_branch_id:'main',branch_id:'new-initial-money',cut_year:1,scenario:changedEndowment},400);
 const changedUnits=structuredClone(without);changedUnits.economy.goods.food.unit='tonelada';
 await call('api/worlds/77001/branches',{parent_branch_id:'main',branch_id:'new-stock-units',cut_year:1,scenario:changedUnits},400);
 await run('main',3);await run('no-shocks',3);
 await run('recipe-six',2);const richer=await view('markets',2,'recipe-six');balanced(richer);assert.equal(group(richer,1,'farmer').goods.food.produced,'6');assert.equal(group(richer,1,'farmer').goods.food.closing_stock,'6');
 const rewrittenPast=structuredClone(scenario);rewrittenPast.scheduled_events[3].name='Baja reescrita';
 await call('api/worlds/77001/branches',{parent_branch_id:'main',branch_id:'rewritten-past',cut_year:3,scenario:rewrittenPast},400);
 const shockYear=await view('markets',2);balanced(shockYear);
 assert.equal(group(shockYear,1,'farmer').goods.food.event_destroyed,'3');assert.equal(group(shockYear,1,'farmer').goods.food.produced,'0');assert.equal(group(shockYear,1,'farmer').goods.food.closing_stock,'0');
 assert.equal(group(shockYear,2).member_count_after,0);assert.equal(group(shockYear,2).closing_money,'8');
 assert.equal((await view('census',2)).total_population,2);
 assert.equal((await view('routes',2)).items.length,0);
 const recovered=await view('markets',3);balanced(recovered);assert.equal(group(recovered,1,'farmer').goods.food.produced,'4');assert.equal((await view('routes',3)).items.length,1);
 const healthy=await view('markets',2,'no-shocks');balanced(healthy);assert.equal(group(healthy,1,'farmer').goods.food.closing_stock,'4');assert.equal((await view('census',2,'no-shocks')).total_population,3);
 assert.deepEqual(await view('markets',1),second);assert.deepEqual(await view('events',1),originalEvents);
 assert.deepEqual((await view('markets',1,'no-shocks')).groups,second.groups);
 const rootEvents=await view('events',3);
 assert.equal(rootEvents.events.filter(e=>e.kind.type==='scheduled_event_occurred').length,4);
 const deaths=await view('people/3',3);assert.equal(deaths.person.person.death_year,2);assert.equal((await view('people/3',1)).person.person.death_year,null);
 assert.equal(rootEvents.events.filter(e=>e.kind.type==='person_died'&&e.kind.data.person_id===3).length,1);
 assert.ok(!(await view('events',3,'no-shocks')).events.some(e=>e.kind.type==='scheduled_event_occurred'));
 const poor=structuredClone(scenario);poor.world_id=77002;poor.scheduled_events=[];poor.economy.initial_group_endowments.forEach(x=>x.endowment.money='0');await call('api/worlds',{scenario:poor});await run('main',0,77002);
 const poorMarket=await view('markets/settlements',0,'main',77002);assert.equal(market(poorMarket,2).unmet_physical_need,'1');assert.equal(market(poorMarket,2).solvent_demand,'0');assert.equal(market(poorMarket,2).next_price,'2');balanced(await view('markets',0,'main',77002));
 const failure=structuredClone(scenario);failure.world_id=77003;failure.scheduled_events=[{...shock(9999,'Exacto imposible',1,{settlement_ids:[2]},[{type:'deaths',data:{selection:exact(99)}}]),start_year:1}];await call('api/worlds',{scenario:failure});
 const job=await call('api/worlds/77003/branches/main/runs',{through_year:1});let failed;
 for(let n=0;n<400;n++){failed=await call(`api/runs/${job.job_id}`);if(failed.status==='failed')break;assert.notEqual(failed.status,'completed');await delay(20);}assert.equal(failed.status,'failed');assert.equal(failed.current_year,0);
 assert.equal((await call('api/worlds/77003/branches'))[0].head_year,0);
 const fixed=structuredClone(failure);fixed.scheduled_events=[];await call('api/worlds/77003/branches',{parent_branch_id:'main',branch_id:'fixed',cut_year:0,scenario:fixed});await run('fixed',1,77003);
 const clamped=structuredClone(failure);clamped.world_id=77004;clamped.scheduled_events[0].effects[0].data.selection.data.overflow='clamp_with_warning';await call('api/worlds',{scenario:clamped});await run('main',1,77004);
 const clampEvents=await view('events',1,'main',77004);const realized=clampEvents.events.find(e=>e.kind.type==='event_effect_realized');assert.equal(realized.kind.data.realization.data.realized_count,1);assert.equal(realized.kind.data.realization.data.warning.kind,'exact_count_clamped');
 const page=await call(route(77001,'main','events',3).replace('limit=100','limit=1'));assert.equal(page.events.length,1);assert.equal(page.total,rootEvents.total);
 const archive=await call('api/worlds/77001/export',{});await call('api/restore',{export_id:archive.export_id});await stop();await start();assert.deepEqual(await view('markets',1),second);assert.equal((await view('census',2,'no-shocks')).total_population,3);
 console.log('E3 black-box acceptance passed: hand-calculated stock/cash, next-year prices, directed cargo-limited trade, physical scarcity vs insolvency, realized losses/death, temporary restriction expiry, exact failure/clamp, immutable past agenda, historical and branch isolation, paginated events, export/restore and restart.');
}finally{await stop();await rm(scratch,{recursive:true,force:true});}
