import path from "node:path"
import { defineConfig, mergeConfig } from "vitest/config"

import shared from "../../vitest.shared"

export default mergeConfig(
  shared,
  defineConfig({
    test: { name: "app" },
    resolve: {
      alias: { "@": path.resolve(import.meta.dirname, "src") },
    },
  }),
)
