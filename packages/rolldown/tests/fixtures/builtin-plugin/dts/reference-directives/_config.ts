import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { dtsPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  // TODO: Reference directive handling works in fake_js (unit tested),
  // but integration needs output filename to be .d.ts for render_chunk to trigger.
  // Skip for now until we add proper entryFileNames handling for .d.ts input.
  skip: true,
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
