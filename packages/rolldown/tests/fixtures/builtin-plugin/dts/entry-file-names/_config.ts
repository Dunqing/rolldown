import { defineTest } from 'rolldown-tests';
import { dtsPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  sequential: true,
  config: {
    input: 'main.ts',
    plugins: [dtsPlugin()],
    output: {
      entryFileNames: 'entries/[name].bundle.js',
      // DTS chunks use chunkFileNames pattern, not entryFileNames
      chunkFileNames: 'types/[name].d.js',
    },
  },
  async afterTest(output) {
    const chunks = output.output.filter((o) => o.type === 'chunk');

    // Check that JS chunk follows the entryFileNames pattern
    const jsChunks = chunks.filter((c) => c.fileName.endsWith('.js'));
    expect(jsChunks.length).toBeGreaterThan(0);
    expect(jsChunks[0].fileName).toContain('entries/');
    expect(jsChunks[0].fileName).toContain('.bundle.js');

    // DTS chunks are emitted via emit_chunk, so they follow chunkFileNames
    // This is a current limitation - they don't follow entryFileNames pattern
    const dtsChunks = chunks.filter((c) => c.fileName.endsWith('.d.ts'));
    expect(dtsChunks.length).toBeGreaterThan(0);
    // DTS chunk should contain the base name 'main' and use .d.ts extension
    expect(dtsChunks[0].fileName).toContain('main');
    expect(dtsChunks[0].fileName).toMatch(/\.d\.ts$/);
  },
});
