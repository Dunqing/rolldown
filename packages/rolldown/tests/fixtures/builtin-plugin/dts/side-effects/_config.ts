import { defineTest } from 'rolldown-tests';
import { dtsPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  sequential: true,
  config: {
    input: 'index.ts',
    plugins: [dtsPlugin({ emitDtsOnly: true })],
    output: {
      preserveModules: true,
    },
  },
  async afterTest(output) {
    const chunks = output.output.filter((o) => o.type === 'chunk');

    // Should have at least one chunk
    expect(chunks.length).toBeGreaterThanOrEqual(1);

    // Find the chunk that contains the global declaration
    const allCode = chunks.map((c) => c.code).join('\n');
    expect(allCode).toContain('declare global');
    expect(allCode).toContain('sideEffectExecuted');
  },
});
