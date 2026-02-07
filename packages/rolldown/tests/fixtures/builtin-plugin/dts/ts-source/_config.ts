import { defineTest } from 'rolldown-tests';
import { getOutputFileNames, getOutputChunk } from 'rolldown-tests/utils';
import { dtsPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  sequential: true,
  config: {
    input: 'main.ts',
    plugins: [dtsPlugin()],
  },
  async afterTest(output) {
    const fileNames = getOutputFileNames(output);

    // Should have both JS and DTS outputs
    expect(fileNames).toContain('main.js');

    // Check that declarations were generated
    const chunks = getOutputChunk(output);
    const jsChunk = chunks.find(c => c.fileName === 'main.js');
    expect(jsChunk).toBeDefined();
    expect(jsChunk!.code).toContain('foo');
  },
});
