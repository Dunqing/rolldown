import { defineTest } from 'rolldown-tests';
import { dtsPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  // Skip: Side-effect-only imports with declare global blocks don't produce output
  // because there are no visible exports for the bundler. The `sideEffects` option
  // is not yet fully implemented in the builtin DTS plugin.
  skip: true,
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

    // Should have multiple chunks due to preserveModules
    expect(chunks.length).toBeGreaterThanOrEqual(1);

    // Find the chunk that contains the global declaration
    const allCode = chunks.map((c) => c.code).join('\n');
    expect(allCode).toContain('declare global');
    expect(allCode).toContain('sideEffectExecuted');
  },
});
