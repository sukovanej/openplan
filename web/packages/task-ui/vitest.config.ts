import { defineConfig, mergeConfig } from "vitest/config"

import shared from "../../vitest.shared"

export default mergeConfig(
  shared,
  defineConfig({
    test: {
      name: "task-ui",
      environment: "happy-dom",
      include: ["tests/**/*.test.{ts,tsx}"],
    },
  }),
)
