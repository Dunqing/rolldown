import { defineTest } from 'rolldown-tests';
import { dtsPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  sequential: true,
  config: {
    input: 'main.ts',
    plugins: [dtsPlugin({ emitDtsOnly: true })],
  },
  async afterTest(output) {
    // With emitDtsOnly: true, there should be no JS chunks
    const chunks = output.output.filter((o) => o.type === 'chunk');
    const jsChunks = chunks.filter((c) => c.fileName.endsWith('.js'));
    const dtsChunks = chunks.filter((c) => c.fileName.endsWith('.d.ts'));

    // Should have no JS output
    expect(jsChunks.length).toBe(0);

    // Should have DTS output
    expect(dtsChunks.length).toBeGreaterThan(0);

    // Verify DTS content
    const dtsContent = dtsChunks[0].code;
    expect(dtsContent).toContain('interface User');
    expect(dtsContent).toContain('createUser');
  },
});
