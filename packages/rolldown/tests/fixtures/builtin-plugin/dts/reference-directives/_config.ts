import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { dtsPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  sequential: true,
  config: {
    input: 'main.d.ts',
    plugins: [dtsPlugin()],
  },
  async afterTest(output) {
    const chunks = getOutputChunk(output);
    const code = chunks[0].code;

    // Reference directives should be preserved
    expect(code).toContain('/// <reference types="node"');
    // Note: path reference might get resolved to absolute
    expect(code).toContain('/// <reference path=');

    // Declarations should be present
    expect(code).toContain('ServerConfig');
    expect(code).toContain('createServer');
  },
});
