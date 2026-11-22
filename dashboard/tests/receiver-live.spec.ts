import { test, expect } from '@playwright/test'
import { readFileSync, writeFileSync } from 'node:fs'
import { resolve } from 'node:path'

const dev = process.env.ENGINEER_TEST_DEV_URL
const receiver = process.env.ENGINEER_TEST_RECEIVER_URL
test.skip(!dev || !receiver, 'Run through scripts/verify-receiver-upgrade.py with an isolated receiver')

test('local proxy supports settings writes and rejects foreign origins', async ({ request }) => {
  const direct = await (await request.get(`${receiver}/api/state`)).json()
  const proxied = await (await request.get(`${dev}/api/state`)).json()
  expect(proxied.event_id).toBe(direct.event_id)
  const saved = await request.post(`${dev}/api/rules`, { data: direct.rules, headers: { Origin: dev! } })
  expect(saved.status()).toBe(200)
  const foreign = await request.post(`${dev}/api/rules`, { data: direct.rules, headers: { Origin: 'https://foreign.invalid' } })
  expect(foreign.status()).toBe(403)
})

test('dashboard code hot reloads while the same collector continues', async ({ page }) => {
  await page.goto(dev!)
  await expect(page.getByRole('heading', { name: 'Fuel & energy', exact: true })).toBeVisible()
  const before = await (await page.request.get(`${dev}/api/state`)).json()
  const source = resolve('dashboard/src/engineer/EngineerApp.tsx')
  const original = readFileSync(source, 'utf8')
  const modified = original.replace("resources:'Fuel & energy'", "resources:'Fuel & energy live edit check'")
  expect(modified).not.toBe(original)
  try {
    writeFileSync(source, modified)
    await expect(page.getByRole('heading', { name: 'Fuel & energy live edit check', exact: true })).toBeVisible({ timeout: 15000 })
    await expect.poll(async () => (await (await page.request.get(`${dev}/api/state`)).json()).live_batches).toBeGreaterThan(before.live_batches)
    const after = await (await page.request.get(`${dev}/api/state`)).json()
    expect(after.event_id).toBe(before.event_id)
    expect(after.sources[0].source_id).toBe(before.sources[0].source_id)
    expect(after.errors).toEqual([])
  } finally {
    // Preserve any unrelated concurrent user edit instead of overwriting it.
    if (readFileSync(source, 'utf8') === modified) writeFileSync(source, original)
  }
  await expect(page.getByRole('heading', { name: 'Fuel & energy', exact: true })).toBeVisible({ timeout: 15000 })
})
