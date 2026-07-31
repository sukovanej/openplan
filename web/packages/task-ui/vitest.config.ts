import { defineConfig, mergeConfig } from "vitest/config"

import shared from "../../vitest.shared"

export default mergeConfig(
  shared,
  defineConfig({
    test: {
      name: "task-ui",
      environment: "happy-dom",
      include: ["tests/**/*.test.{ts,tsx}"],
      // The package is a scaffold until OPP-63 fills it; selecting this project alone must not fail.
      passWithNoTests: true,
    },
  }),
)
