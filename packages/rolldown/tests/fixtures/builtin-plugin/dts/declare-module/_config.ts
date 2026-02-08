import { defineTest } from 'rolldown-tests';
import { dtsPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  sequential: true,
  config: {
    input: 'main.ts',
    platform: 'node',
    plugins: [dtsPlugin({ emitDtsOnly: true })],
  },
  async afterTest(output) {
    const chunks = output.output.filter((o) => o.type === 'chunk');
    const dtsChunk = chunks.find((c) => c.fileName.endsWith('.d.ts'));

    expect(dtsChunk).toBeDefined();
    // Should preserve declare module statement
    expect(dtsChunk!.code).toContain('declare module');
    // Check for module name (quotes may be single or double)
    expect(dtsChunk!.code).toMatch(/["']virtual["']/);
  },
});
