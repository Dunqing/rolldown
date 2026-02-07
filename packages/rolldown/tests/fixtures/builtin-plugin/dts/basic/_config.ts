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
    // Check that output contains the bundled declaration file
    const fileNames = getOutputFileNames(output);
    console.log('Output file names:', fileNames);

    const chunks = getOutputChunk(output);
    console.log('Chunk code:', JSON.stringify(chunks[0].code));

    expect(fileNames).toMatchInlineSnapshot(`
      [
        "main.d.ts",
      ]
    `);

    // The output should contain the bundled types (reconstructed from fake JS)
    const chunk = chunks[0];
    expect(chunk.code).toContain('User');
    expect(chunk.code).toContain('UserId');
    expect(chunk.code).toContain('App');
  },
});
