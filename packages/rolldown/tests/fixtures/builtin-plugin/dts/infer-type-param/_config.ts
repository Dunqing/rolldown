import { defineTest } from 'rolldown-tests';
import { dtsPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  sequential: true,
  config: {
    input: 'main.ts',
    plugins: [dtsPlugin({ emitDtsOnly: true })],
  },
  async afterTest(output) {
    const chunks = output.output.filter((o) => o.type === 'chunk');
    const dtsChunk = chunks.find((c) => c.fileName.endsWith('.d.ts'));

    expect(dtsChunk).toBeDefined();
    // Ensure type parameter U is not renamed to U$1
    expect(dtsChunk!.code).toContain('Fn1<U = unknown>');
    expect(dtsChunk!.code).not.toContain('U$1');
  },
});
