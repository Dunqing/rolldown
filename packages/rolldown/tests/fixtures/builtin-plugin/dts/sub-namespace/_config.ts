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
    // Should contain namespace with dot notation
    // Output uses inline `export declare namespace` format
    expect(dtsChunk!.code).toContain('namespace Foo.Bar');
  },
});
