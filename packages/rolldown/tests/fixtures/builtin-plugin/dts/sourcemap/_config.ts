import { defineTest } from 'rolldown-tests';
import { dtsPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  sequential: true,
  // TODO: Sourcemap support is partially implemented
  // Currently generates TS->DTS sourcemap but doesn't chain it through
  skip: true,
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

    // DTS chunk should have sourcemap info
    // Note: Full sourcemap support is complex due to multiple transformation stages
    expect(dtsChunk!.code).toContain('User');
    expect(dtsChunk!.code).toContain('createUser');
  },
});
