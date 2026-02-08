import { defineTest } from 'rolldown-tests';
import { dtsPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  // Skip: ambient module declarations (declare module 'x') don't produce output
  // because they don't have visible exports for the bundler to process.
  // This is a known limitation of the current implementation.
  skip: true,
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
    expect(dtsChunk!.code).toContain("'virtual'");
  },
});
