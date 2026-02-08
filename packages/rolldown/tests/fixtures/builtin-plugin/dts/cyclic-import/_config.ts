import { defineTest } from 'rolldown-tests';
import { dtsPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  sequential: true,
  config: {
    input: ['a.ts', 'b.ts'],
    plugins: [dtsPlugin({ emitDtsOnly: true })],
  },
  async afterTest(output) {
    const chunks = output.output.filter((o) => o.type === 'chunk');

    // Should have at least one DTS chunk
    const dtsChunks = chunks.filter((c) => c.fileName.endsWith('.d.ts'));
    expect(dtsChunks.length).toBeGreaterThanOrEqual(1);

    // Get all DTS code combined
    const allDtsCode = dtsChunks.map((c) => c.code).join('\n');

    // Should contain types from both files (they may be consolidated)
    expect(allDtsCode).toContain('SomeInterface');
    expect(allDtsCode).toContain('SomeBoolean');
    expect(allDtsCode).toContain('SomeClass');
  },
});
