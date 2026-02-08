import { defineTest } from 'rolldown-tests';
import { dtsPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  sequential: true,
  config: {
    input: 'main.ts',
    plugins: [dtsPlugin({ sourcemap: true })],
    output: {
      sourcemap: true,
    },
  },
  async afterTest(output) {
    const chunks = output.output.filter((o) => o.type === 'chunk');

    // Should have JS and DTS chunks
    const dtsChunk = chunks.find((c) => c.fileName.endsWith('.d.ts'));
    expect(dtsChunk).toBeDefined();

    // DTS chunk should contain our types
    expect(dtsChunk!.code).toContain('User');
    expect(dtsChunk!.code).toContain('createUser');

    // DTS chunk should have a sourcemap
    expect(dtsChunk!.map).toBeDefined();
    expect(dtsChunk!.map?.sources).toBeDefined();
    expect(dtsChunk!.map?.sources.length).toBeGreaterThan(0);
  },
});
