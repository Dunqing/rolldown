import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { dtsPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  sequential: true,
  config: {
    input: 'index.d.ts',
    plugins: [dtsPlugin()],
    treeshake: true,
  },
  async afterTest(output) {
    const chunks = getOutputChunk(output);
    const code = chunks[0].code;

    // Should contain type A (exported)
    expect(code).toContain('A');

    // Type B and function foo should be tree-shaken away
    // (not exported from index.d.ts)
    expect(code).not.toContain('foo');
    expect(code).not.toContain('B');
  },
});
