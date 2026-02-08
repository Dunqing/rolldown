import { defineTest } from 'rolldown-tests';
import { dtsPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  sequential: true,
  config: {
    input: 'index.ts',
    plugins: [dtsPlugin()],
  },
  async afterTest(output) {
    const chunks = output.output.filter((o) => o.type === 'chunk');
    const dtsChunk = chunks.find((c) => c.fileName.endsWith('.d.ts'));

    expect(dtsChunk).toBeDefined();
    // Should contain the resolved type from mod.d.ts
    expect(dtsChunk!.code).toContain('Foo');
  },
});
