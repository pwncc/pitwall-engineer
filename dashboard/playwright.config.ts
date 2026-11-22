import { defineConfig } from '@playwright/test'

export default defineConfig({
  use: { launchOptions: { args: ['--disable-gpu'] } },
})
