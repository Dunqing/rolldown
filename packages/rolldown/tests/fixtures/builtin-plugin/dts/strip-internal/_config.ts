import { defineTest } from 'rolldown-tests';
import { dtsPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  sequential: true,
  config: {
    input: 'main.ts',
    plugins: [dtsPlugin({ stripInternal: true })],
  },
  async afterTest(output) {
    const chunks = output.output.filter((o) => o.type === 'chunk');
    const dtsChunks = chunks.filter((c) => c.fileName.endsWith('.d.ts'));

    expect(dtsChunks.length).toBeGreaterThan(0);
    const dtsContent = dtsChunks[0].code;

    // Public declarations should be present
    expect(dtsContent).toContain('interface Config');
    expect(dtsContent).toContain('getConfig');

    // @internal declarations should be stripped
    expect(dtsContent).not.toContain('InternalConfig');
    expect(dtsContent).not.toContain('internalHelper');
    expect(dtsContent).not.toContain('debugMode');
    expect(dtsContent).not.toContain('secretKey');
  },
});
