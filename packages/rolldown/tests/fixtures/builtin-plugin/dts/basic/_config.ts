import { defineTest } from 'rolldown-tests';
import { getOutputFileNames, getOutputChunk } from 'rolldown-tests/utils';
import { dtsPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  sequential: true,
  config: {
    input: 'main.d.ts',
    plugins: [dtsPlugin()],
  },
  async afterTest(output) {
    // Check that output contains the bundled file
    // Note: The plugin transforms .d.ts to fake JS for bundling
    expect(getOutputFileNames(output)).toMatchInlineSnapshot(`
      [
        "main.d.js",
      ]
    `);

    // The output should contain the bundled types
    const chunk = getOutputChunk(output);
    expect(chunk[0].code).toContain('User');
    expect(chunk[0].code).toContain('UserId');
    expect(chunk[0].code).toContain('App');
  },
});
