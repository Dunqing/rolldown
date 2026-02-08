import { defineTest } from 'rolldown-tests';
import { dtsPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  sequential: true,
  config: {
    input: 'index.ts',
    plugins: [dtsPlugin({ emitDtsOnly: true })],
  },
  async afterTest(output) {
    const chunks = output.output.filter((o) => o.type === 'chunk');
    const dtsChunks = chunks.filter((c) => c.fileName.endsWith('.d.ts'));

    expect(dtsChunks.length).toBeGreaterThanOrEqual(1);

    // Get all DTS code combined
    const allDtsCode = dtsChunks.map((c) => c.code).join('\n');

    // Should contain the declarations (class A and B from mod.ts)
    expect(allDtsCode).toContain('class A');
    expect(allDtsCode).toContain('class B');
    // Should contain ns and all from the other files
    expect(allDtsCode).toContain('ns');
    expect(allDtsCode).toContain('all');
  },
});
