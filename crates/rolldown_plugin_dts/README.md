# rolldown_plugin_dts

A Rolldown builtin plugin that generates and bundles TypeScript `.d.ts` declaration files. It leverages Rolldown's existing JavaScript bundling infrastructure — including tree-shaking, code splitting, and sourcemaps — for type declarations.

## Usage

```typescript
import { dtsPlugin } from 'rolldown/experimental';

export default {
  input: 'src/index.ts',
  plugins: [
    dtsPlugin({
      stripInternal: true,
      emitDtsOnly: true,
      sourcemap: true,
    }),
  ],
};
```

## Options

| Option          | Type      | Default | Description                                              |
| --------------- | --------- | ------- | -------------------------------------------------------- |
| `stripInternal` | `boolean` | `false` | Remove declarations annotated with `@internal` JSDoc tag |
| `emitDtsOnly`   | `boolean` | `false` | Remove JS chunks from output, keep only `.d.ts` files    |
| `sourcemap`     | `boolean` | `false` | Generate `.d.ts.map` sourcemaps for declaration files    |

## How It Works

The plugin uses a **fake JS pipeline** to convert TypeScript declarations into JavaScript that Rolldown can bundle, then converts them back.

### Pipeline

```
  .ts source
      |
      v
  Oxc IsolatedDeclarations  -->  .d.ts
      |
      v
  Fake JS encoding  -->  var Foo = [id, () => [deps], ["Foo"], "source", "file", line];
      |
      v
  Rolldown bundling  (tree-shaking, code splitting, etc.)
      |
      v
  Fake JS decoding  -->  .d.ts output
```

Each TypeScript declaration is encoded as a JavaScript variable containing:

- A unique numeric ID for deduplication
- A closure returning dependency names (enables Rolldown's dependency tracking)
- The original declaration source as a string literal
- Source file and line info for sourcemap reconstruction

This allows Rolldown to treat type declarations as regular JavaScript modules — tracking dependencies, applying tree-shaking to remove unused types, and splitting shared types across chunks.

### Plugin Hooks

1. **transform** — Captures `.ts`/`.tsx` sources and emits virtual DTS entry chunks. Converts `.d.ts` code to fake JS for bundling.
2. **resolveId** — Resolves virtual DTS module IDs (`\0dts:...`) and rewrites import paths within the DTS context.
3. **load** — Generates `.d.ts` from captured TypeScript sources using Oxc's `IsolatedDeclarations`.
4. **generateBundle** — Converts bundled fake JS back to `.d.ts`, renames output files, and optionally removes JS chunks.

## Supported Features

- **Declaration generation** from `.ts`, `.tsx`, `.mts`, `.cts` via Oxc IsolatedDeclarations
- **Tree-shaking** of unused type declarations
- **Code splitting** with shared type chunks across multiple entries
- **Manual chunks** via `manualChunks` configuration
- **Sourcemaps** mapping bundled `.d.ts` back to original source files
- **Reference directives** (`/// <reference types="..." />`, `/// <reference path="..." />`)
- **Ambient modules** (`declare module 'name' { ... }`)
- **Global augmentations** (`declare global { ... }`)
- **Type-only exports** (`export type { Foo }`, `export type * as ns from '...'`)
- **Re-exports** (`export * from '...'`, `export { Foo } from '...'`)
- **Cyclic dependencies** between type modules
- **External types** — imports from `node_modules` are preserved, not inlined
- **Existing `.d.ts` inputs** — resolves and bundles pre-written declaration files
- **`@internal` stripping** — removes declarations tagged with `@internal`
- **Namespace declarations** including dotted names (`namespace Foo.Bar {}`)
- **Entry file name mapping** — `.js` → `.d.ts`, `.mjs` → `.d.mts`, `.cjs` → `.d.cts`

## Architecture

```
crates/rolldown_plugin_dts/
  src/
    lib.rs        # Public exports (DtsPlugin, DtsPluginOptions)
    options.rs    # Configuration structs
    generate.rs   # Plugin hook implementations (transform, resolveId, load, generateBundle)
    fake_js.rs    # Fake JS encoding/decoding pipeline
    resolver.rs   # Import path resolution for DTS context
    utils.rs      # Virtual ID management, path conversions
```

### Virtual Module IDs

The plugin uses virtual module IDs with the `\0dts:` prefix:

```
Source file:    /src/types.ts
Virtual DTS:   \0dts:/src/types.d.ts
Output file:   types.d.ts
```

### Special Markers

| Marker                | Purpose                                       |
| --------------------- | --------------------------------------------- |
| `__DTS_REF__`         | Reference directive (`/// <reference ... />`) |
| `__DTS_AMBIENT__`     | Ambient module or global declaration          |
| `__DTS_PASSTHROUGH__` | Re-export or other pass-through content       |

## Example

**Input** (`src/index.ts`):

```typescript
export interface User {
  id: number;
  name: string;
}

export function createUser(name: string): User {
  return { id: 1, name };
}
```

**Output** (`dist/index.d.ts`):

```typescript
export interface User {
  id: number;
  name: string;
}
export declare function createUser(name: string): User;
```

With `emitDtsOnly: false`, both `dist/index.js` and `dist/index.d.ts` are emitted. With `emitDtsOnly: true`, only the `.d.ts` file is kept.
