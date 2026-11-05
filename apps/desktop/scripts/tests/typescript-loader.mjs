import { existsSync, readFileSync } from "node:fs"
import { fileURLToPath } from "node:url"
import ts from "typescript"

export function resolve(specifier, context, next) {
  if (specifier.startsWith(".") && context.parentURL) {
    const url = new URL(`${specifier}.ts`, context.parentURL)
    if (existsSync(fileURLToPath(url))) return { url: url.href, shortCircuit: true }
  }
  return next(specifier, context)
}

export function load(url, context, next) {
  if (url.endsWith(".ts") && !url.includes("node_modules")) return {
    format: "module", shortCircuit: true,
    source: ts.transpileModule(readFileSync(new URL(url), "utf8"), {
      compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext },
    }).outputText,
  }
  return next(url, context)
}
