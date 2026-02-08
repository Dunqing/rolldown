import { defineTest } from 'rolldown-tests';
import { dtsPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  sequential: true,
  config: {
    input: 'main.ts',
    plugins: [dtsPlugin()],
    external: ['rolldown'],
  },
  async afterTest(output) {
    const chunks = output.output.filter((o) => o.type === 'chunk');

    // Should have JS and DTS chunks
    const jsChunk = chunks.find((c) => c.fileName.endsWith('.js'));
    const dtsChunk = chunks.find((c) => c.fileName.endsWith('.d.ts'));

    expect(jsChunk).toBeDefined();
    expect(dtsChunk).toBeDefined();

    // DTS should have our interface and function
    expect(dtsChunk!.code).toContain('MyOutput');
    expect(dtsChunk!.code).toContain('processOutput');

    // External import should be preserved (not inlined)
    expect(dtsChunk!.code).toContain('RolldownOutput');
  },
});
