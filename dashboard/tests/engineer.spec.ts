import {test,expect} from '@playwright/test'

test('shared sender package is the default for co-drivers', async ({ page }) => {
 let body: { shared_package?: boolean; entry_id?: string } | undefined
 await page.route('**/api/pair', route => { body = route.request().postDataJSON(); return route.fulfill({ status: 400, body: 'Pair request captured' }) })
 await page.goto('http://127.0.0.1:17437')
 await page.getByRole('button', { name: 'Setup & event rules', exact: true }).click()
 await expect(page.getByLabel('One sender EXE for all co-drivers (automatic PC identity)')).toBeChecked()
 await page.getByRole('button', { name: 'Download sender', exact: true }).click()
 await expect.poll(() => body?.shared_package).toBe(true)
 expect(body?.entry_id).toBe('Car 1')
})

test('engineer can inspect feeds and configure strategy',async({page})=>{
 const errors:string[]=[]
 page.on('pageerror',e=>errors.push(e.message))
 await page.goto('http://127.0.0.1:17437')
 await expect(page.getByRole('heading',{name:'Race Overview',exact:true})).toBeVisible()
 await expect(page.getByText('Receiver online',{exact:true})).toBeVisible()
 await expect(page.getByRole('heading',{name:'Team cars',exact:true})).toBeVisible()
 await page.screenshot({path:'../test_data/overview.png',fullPage:true})
 await page.getByRole('button',{name:'Car Engineering',exact:true}).click()
 await expect(page.getByRole('heading',{name:'Tyres & brakes'})).toBeVisible()
 await page.getByRole('button',{name:'Edit layout',exact:true}).click()
 await page.getByRole('button',{name:'Add panel',exact:true}).click()
 await page.getByRole('button',{name:'Track map',exact:true}).click()
 await expect(page.getByRole('heading',{name:'Track map',exact:true})).toBeVisible()
 await page.reload()
 await page.getByRole('button',{name:'Car Engineering',exact:true}).click()
 await expect(page.getByRole('heading',{name:'Track map',exact:true})).toBeVisible()
 await page.getByRole('button',{name:'Strategy',exact:true}).click()
 await expect(page.getByRole('heading',{name:'Pit strategy'})).toBeVisible()
 await page.getByRole('button',{name:'Setup & event rules',exact:true}).click()
 await page.getByRole('button',{name:'Event & strategy',exact:true}).click()
 await page.getByLabel('Pit transit loss · seconds',{exact:true}).fill('38')
 await page.getByRole('button',{name:'Save event settings',exact:true}).click()
 await expect(page.getByRole('status')).toHaveText('Event rules saved.')
 await page.getByRole('button',{name:'Close setup',exact:true}).click()
 await page.getByRole('button',{name:'Performance',exact:true}).click()
 await expect(page.getByRole('heading',{name:'Telemetry'})).toBeVisible()
 await page.screenshot({path:'../test_data/performance.png',fullPage:true})
 expect(errors).toEqual([])
})
test('dashboard remains usable at a narrow viewport',async({page})=>{
 await page.setViewportSize({width:800,height:900})
 await page.goto('http://127.0.0.1:17437')
 await expect(page.getByRole('heading',{name:'Race Overview',exact:true})).toBeVisible()
 await expect.poll(async()=>Math.max(...await page.locator('.eng-panel').evaluateAll(nodes=>nodes.map(n=>n.getBoundingClientRect().right)))).toBeLessThanOrEqual(800)
 await page.screenshot({path:'../test_data/narrow.png',fullPage:true})
 expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBeTruthy()
 const edges=await page.locator('.eng-panel').evaluateAll(nodes=>nodes.map(n=>n.getBoundingClientRect().right))
 expect(Math.max(...edges)).toBeLessThanOrEqual(800)
})


test('recorded lap loads without a reference selection',async({page})=>{
 await page.goto('http://127.0.0.1:17437')
 const state=await (await page.request.get('http://127.0.0.1:17437/api/state')).json()
 const entry=state.entries.find((e:{laps:unknown[]})=>e.laps.length>0)
 expect(entry,'fixture receiver must contain recorded laps').toBeTruthy()
 await page.getByLabel('FOCUS',{exact:false}).selectOption(entry.entry_id)
 await page.getByRole('button',{name:'Performance',exact:true}).click()
 const select=page.getByRole('combobox',{name:'Current lap',exact:true})
 await expect(select.locator('option')).not.toHaveCount(1)
 const value=await select.locator('option').nth(1).getAttribute('value')
 await select.selectOption(value!)
 await expect(page.getByText(/Recorded lap/)).toBeVisible()
 await expect(page.getByRole('combobox',{name:'Reference lap',exact:true})).toHaveValue('')
 await page.screenshot({path:'../test_data/recorded-lap.png',fullPage:true})
 const delayed=page.waitForRequest(req=>req.url().includes('/api/replay/')&&new URL(req.url()).searchParams.get('limit')==='480')
 await page.getByRole('spinbutton',{name:/Stream delay/}).fill('300')
 await delayed
 await expect(select).toHaveValue('')
})

test('imported reference is immediately available without changing live lap evidence',async({page})=>{
 await page.goto('http://127.0.0.1:17437')
 const state=await (await page.request.get('http://127.0.0.1:17437/api/state')).json()
 const entry=state.entries.find((e:{laps:unknown[]})=>e.laps.length>0)
 expect(entry).toBeTruthy()
 const lap=await (await page.request.get(`http://127.0.0.1:17437/api/lap/${entry.laps[0].id}`)).json()
 lap.driver=`Imported UI reference ${Date.now()}`
 await page.getByRole('combobox',{name:'FOCUS',exact:true}).selectOption(entry.entry_id)
 await page.getByRole('button',{name:'Performance',exact:true}).click()
 await page.locator('input[type="file"]').setInputFiles({name:'reference.json',mimeType:'application/json',buffer:Buffer.from(JSON.stringify(lap))})
 const references=page.getByRole('combobox',{name:'Reference lap',exact:true})
 await expect(references.locator('option').filter({hasText:lap.driver})).toHaveCount(1)
 const after=await (await page.request.get('http://127.0.0.1:17437/api/state')).json()
 expect(after.entries.find((e:{entry_id:string})=>e.entry_id===entry.entry_id).laps.length).toBe(entry.laps.length)
})


test('a chosen reference overlays the live partial lap',async({page})=>{
 await page.goto('http://127.0.0.1:17437')
 const state=await (await page.request.get('http://127.0.0.1:17437/api/state')).json()
 const entry=state.entries.find((e:{laps:unknown[]})=>e.laps.length>0)
 await page.getByRole('combobox',{name:'FOCUS',exact:true}).selectOption(entry.entry_id)
 await page.getByRole('button',{name:'Performance',exact:true}).click()
 const references=page.getByRole('combobox',{name:'Reference lap',exact:true})
 const value=await references.locator('option').nth(1).getAttribute('value')
 await references.selectOption(value!)
 await expect(page.getByText(/Live lap vs reference/)).toBeVisible()
 await expect(page.locator('.eng-map polyline[stroke="#a398ff"]')).toBeVisible()
 await expect(page.getByRole('combobox',{name:'Current lap',exact:true})).toHaveValue('')
})


test('track history does not join different sessions with the same lap number',async({page})=>{
 const state=await (await page.request.get('http://127.0.0.1:17437/api/state')).json()
 const base=state.entries[0].last_sample
 const now=Date.now()-10000
 const points=[0,1,2,3,4,5].map(i=>({...base,captured_ms:now+i*100,game_s:i<3?100+i:i-3,track:i<3?'Old session':'New session',lap:0,x:i<3?1000+i*100:i-3,z:i<3?1000+i*100:(i-3)*2}))
 await page.route('**/api/replay/**',route=>route.fulfill({json:points}))
 await page.goto('http://127.0.0.1:17437')
 const line=page.locator('.eng-map polyline[stroke="#55e1b5"]')
 await expect.poll(async()=>{const p=await line.getAttribute('points');return p?.trim().split(' ').length||0}).toBe(3)
})

test('panels can be added resized removed and saved without telemetry or a receiver',async({page})=>{
 const errors:string[]=[]
 page.on('pageerror',e=>errors.push(e.message))
 const state=await (await page.request.get('http://127.0.0.1:17437/api/state')).json()
 await page.route('**/api/state',route=>route.fulfill({json:{...state,entries:[],sources:[]}}))
 await page.goto('http://127.0.0.1:17437')
 await expect(page.getByRole('heading',{name:'Fuel & energy',exact:true})).toBeVisible()
 await expect(page.getByText(/Make the call|Bring the team|Engineer advice only/)).toHaveCount(0)
 await page.getByRole('button',{name:'Edit layout',exact:true}).click()
 await page.getByRole('button',{name:'Add panel',exact:true}).click()
 await page.getByRole('button',{name:'Tyres & brakes',exact:true}).click()
 await page.getByRole('button',{name:'Remove Timing',exact:true}).click()
 await expect(page.getByRole('heading',{name:'Timing',exact:true})).toHaveCount(0)
 const item=page.locator('.react-grid-item').filter({has:page.getByRole('heading',{name:'Tyres & brakes',exact:true})})
 const before=await item.boundingBox()
 const handle=item.locator('.react-resizable-handle-se')
 await handle.scrollIntoViewIfNeeded()
 const box=await handle.boundingBox()
 await page.mouse.move(box!.x+box!.width/2,box!.y+box!.height/2)
 await page.mouse.down()
 await page.mouse.move(box!.x+box!.width/2,box!.y+box!.height/2+70,{steps:8})
 await page.mouse.up()
 await expect.poll(async()=>(await item.boundingBox())!.height).toBeGreaterThan(before!.height)
 await page.route('**/api/state',route=>route.abort())
 await expect(page.getByText('Receiver offline',{exact:true})).toBeVisible()
 await page.getByRole('button',{name:'Done editing',exact:true}).click()
 const height=await item.evaluate(node=>(node as HTMLElement).style.height)
 await page.reload()
 await expect(page.getByRole('heading',{name:'Tyres & brakes',exact:true})).toBeVisible()
 await expect(page.getByRole('heading',{name:'Timing',exact:true})).toHaveCount(0)
 await expect.poll(()=>item.evaluate(node=>(node as HTMLElement).style.height)).toBe(height)
 await page.screenshot({path:'../test_data/offline-layout.png',fullPage:true})
 expect(errors).toEqual([])
})

test('paired cars can be assigned to panels before they send telemetry',async({page})=>{
 const state=await (await page.request.get('http://127.0.0.1:17437/api/state')).json()
 await page.route('**/api/state',route=>route.fulfill({json:{...state,entries:[],sources:[{source_id:'offline-pair',entry_id:'Car 27',label:'Driver PC',revoked:false}]}}))
 await page.goto('http://127.0.0.1:17437')
 await page.getByRole('combobox',{name:'Fuel & energy car',exact:true}).selectOption('Car 27')
 await page.reload()
 await expect(page.getByRole('combobox',{name:'Fuel & energy car',exact:true})).toHaveValue('Car 27')
 await expect(page.getByRole('combobox',{name:'FOCUS',exact:true})).toHaveValue('Car 27')
})

test('collector connection is visible before the game sends telemetry',async({page})=>{
 const state=await (await page.request.get('http://127.0.0.1:17437/api/state')).json()
 let online=true
 await page.route('**/api/state',route=>route.fulfill({json:{...state,entries:[],sources:[{source_id:'waiting-pc',entry_id:'Car 9',label:'Driver PC',revoked:false,last_seen_ms:Date.now(),online}]}}))
 await page.goto('http://127.0.0.1:17437')
 await expect(page.getByText('Collector connected · no telemetry',{exact:true})).toBeVisible()
 await page.getByRole('button',{name:'Connect a driver',exact:true}).click()
 const source=page.locator('.eng-source').filter({hasText:'Driver PC'})
 await expect(source).toContainText('Car 9 · Connected')
 online=false
 await expect(source).toContainText('Car 9 · Disconnected')
})

test('weather forecast and brake thickness expire when the collector disconnects',async({page})=>{
 const state=await (await page.request.get('http://127.0.0.1:17437/api/state')).json()
 const entry=state.entries[0]
 let connected=true
 await page.route('**/api/state',route=>{
  const now=Date.now()
  return route.fulfill({json:{...state,entries:[{...entry,last_received_ms:connected?now:now-60000,last_sample:{...entry.last_sample,authoritative:true,captured_ms:now,supplemental:{observed_ms:now,wearables:{weatherForecast:{nodes:{Temperature:[29,30,30,30,31],RainChance:[0,31,0,53,0],WindSpeed:[12,11,10,11,6]}},wearables:{brakes:[.04,.04,.035,.035]}}}}}]}})
 })
 await page.goto('http://127.0.0.1:17437')
 await page.getByRole('button',{name:'Car Engineering',exact:true}).click()
 const weather=page.locator('.react-grid-item').filter({has:page.getByRole('heading',{name:'Weather',exact:true})})
 await expect(weather.getByText('40.0 / 40.0 / 35.0 / 35.0 mm',{exact:true})).toBeVisible()
 await expect(weather.locator('tbody tr')).toHaveCount(5)
 await expect(weather.getByText('43.2 km/h',{exact:true})).toBeVisible()
 await expect(weather.getByText('31%',{exact:true})).toBeVisible()
 await expect(weather.getByText('53%',{exact:true})).toBeVisible()
 connected=false
 await expect(weather.getByText('Unavailable / stale',{exact:true})).toBeVisible()
 await expect(weather.locator('tbody tr')).toHaveCount(0)
 await expect(weather.getByText('Brake thickness',{exact:true})).toHaveCount(0)
})

test('legacy electronics stay unavailable while supported readings remain visible',async({page})=>{
 const state=await(await page.request.get('http://127.0.0.1:17437/api/state')).json()
 const entry=state.entries[0]
 await page.route('**/api/state',route=>route.fulfill({json:{...state,entries:[{...entry,last_received_ms:Date.now(),last_sample:{...entry.last_sample,authoritative:true,soc:0,tc:0,brake_bias_front:.4875,supplemental:{adapter:'rF2 SMMP 3.7',capabilities:{virtual_energy:false,battery_soc:false,native_electronics:false}}}}]}}))
 await page.goto('http://127.0.0.1:17437')
 await page.getByRole('button',{name:'Car Engineering',exact:true}).click()
 const systems=page.locator('.react-grid-item').filter({has:page.getByRole('heading',{name:'Car systems',exact:true})})
 await expect(systems.locator('.eng-system-grid>div').filter({hasText:'BATTERY'}).locator('b')).toHaveText('—%')
 await expect(systems.locator('.eng-system-grid>div').filter({hasText:'TRACTION CONTROL'}).locator('b')).toHaveText('—')
 await expect(systems.locator('.eng-system-grid>div').filter({hasText:'FRONT BRAKE BIAS'}).locator('b')).toHaveText('48.8%')
})
