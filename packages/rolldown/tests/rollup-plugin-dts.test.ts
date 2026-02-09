import fs from 'node:fs';
import path from 'node:path';
import { test, expect } from 'vitest';
import { rolldown } from 'rolldown';
import { dtsPlugin } from 'rolldown/experimental';

const fixturesDir = path.resolve(
  import.meta.dirname,
  'fixtures/builtin-plugin/dts/rollup-plugin-dts',
);

// Find all test directories with index.d.ts or main-a.d.ts entry files
function findEntryFiles(dir: string): string[] {
  const entries: string[] = [];
  for (const name of fs.readdirSync(dir)) {
    const fullPath = path.join(dir, name);
    if (!fs.statSync(fullPath).isDirectory()) continue;
    const indexEntry = path.join(fullPath, 'index.d.ts');
    const mainAEntry = path.join(fullPath, 'main-a.d.ts');
    if (fs.existsSync(indexEntry)) {
      entries.push(indexEntry);
    } else if (fs.existsSync(mainAEntry)) {
      entries.push(mainAEntry);
    }
    // Recurse into subdirectories (for error/ subdirectory)
    const subEntries = findEntryFiles(fullPath);
    entries.push(...subEntries);
  }
  return entries;
}

const entryFiles = findEntryFiles(fixturesDir);

// Tests that need features not supported by the builtin plugin
const skipTests = new Set([
  'path-mapping', // needs tsconfig paths resolution
  'multiple-entries-multiple-tsconfigs', // needs per-entry tsconfig
  'reference-path-remapping', // needs tsconfig-based path resolution
  'reference-path-remapping-should-not-touch-absolute-path', // needs tsconfig-based path resolution
  'issue-101-allow-js', // needs allowJs tsconfig option
  'inline-external-node-next-module', // needs moduleResolution: nodenext
  // The following need fake_js to handle TS declaration merging (same name exported as both type and value)
  'issue-87', // interface + const with same name (declaration merging)
  'issue-284', // interface + namespace with same name (declaration merging)
  'issue-89-import-equals', // import = require() syntax not supported in fake_js
  'namespace-definition', // function + namespace with same name (declaration merging)
  'namespace-definition-rename', // function + namespace with same name
  'overrides-with-rename', // duplicate exports from declaration merging
  // Resolution edge cases
  'circular-to-entry', // import from "." causes resolution hang
]);

for (const entryFile of entryFiles) {
  const dirname = path.dirname(entryFile);
  const testName = path.relative(fixturesDir, dirname);

  const isSkipped = skipTests.has(testName);
  const isError = testName.startsWith('error');

  const testFn = isSkipped ? test.skip : test;

  testFn(testName, async () => {
    let entries = [entryFile];
    if (entryFile.endsWith('main-a.d.ts')) {
      entries = fs
        .readdirSync(dirname)
        .filter((f) => f.startsWith('main-') && f.endsWith('.d.ts'))
        .map((f) => path.join(dirname, f));
    }

    const input: Record<string, string> = {};
    for (const entry of entries) {
      const name = path.basename(entry, path.extname(entry));
      input[name] = entry;
    }

    const build = async () => {
      const bundle = await rolldown({
        input,
        plugins: [dtsPlugin()],
        resolve: {
          tsconfigFilename: path.join(fixturesDir, 'tsconfig.json'),
        },
        treeshake: true,
        // Mark all bare module specifiers as external (non-relative, non-absolute)
        external: (id) => !id.startsWith('.') && !id.startsWith('/'),
      });
      const { output } = await bundle.generate({
        format: 'esm',
        sourcemap: false,
        entryFileNames: '[name].d.ts',
        chunkFileNames: '[name].d.ts',
      });
      return output;
    };

    if (isError) {
      await expect(build()).rejects.toThrow();
      return;
    }

    const output = await build();

    const chunks = output
      .filter(
        (item): item is Extract<typeof item, { type: 'chunk' }> =>
          item.type === 'chunk',
      )
      .sort((a, b) => a.fileName.localeCompare(b.fileName));

    const snapshot = chunks
      .map((chunk) => `// ${chunk.fileName}\n${chunk.code}`)
      .join('\n');

    await expect(snapshot).toMatchFileSnapshot(
      path.resolve(dirname, 'snapshot.d.ts'),
    );
  });
}
