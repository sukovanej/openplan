import { readFileSync, writeFileSync } from "node:fs"

const [path] = process.argv.slice(2)
const namesByBarrel = new Map<string, Array<string>>()

const body = readFileSync(path, "utf8")
  .replace(/^import type \{ SchemaError \} from "effect\/Schema"\n/m, "")
  .replace(/(?<![\w.])SchemaError\b/g, "Schema.SchemaError")
  .replace(/^import (type )?\* as (\w+) from "(effect(?:\/[a-z-]+)?)\/\2"\n/gm, (_, typeOnly, name, barrel) => {
    namesByBarrel.set(barrel, [...(namesByBarrel.get(barrel) ?? []), typeOnly ? `type ${name}` : name])
    return ""
  })

const imports = [...namesByBarrel].map(([barrel, names]) => `import { ${names.join(", ")} } from "${barrel}"\n`)
writeFileSync(path, imports.join("") + body)
