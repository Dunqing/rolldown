import { defineTest } from 'rolldown-tests';
import { dtsPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  sequential: true,
  config: {
    input: 'entry.ts',
    plugins: [dtsPlugin({ emitDtsOnly: true })],
    output: {
      manualChunks(id) {
        if (id.includes('shared1')) return 'shared1-chunk.d'
      },
    },
  },
  async afterTest(output) {
    const chunks = output.output.filter((o) => o.type === 'chunk');
    const dtsChunks = chunks.filter((c) => c.fileName.endsWith('.d.ts'));

    // Should have at least 2 DTS chunks (manualChunks splits them)
    expect(dtsChunks.length).toBeGreaterThanOrEqual(2);

    // Find the shared1-chunk
    const sharedChunk = dtsChunks.find((c) => c.fileName.includes('shared1-chunk'));
    expect(sharedChunk).toBeDefined();
    // shared1-chunk should contain shared1 declaration
    expect(sharedChunk!.code).toContain('shared1');

    // All chunks combined should contain both shared1 and shared2
    const allCode = dtsChunks.map((c) => c.code).join('\n');
    expect(allCode).toContain('shared1');
    expect(allCode).toContain('shared2');
  },
});
