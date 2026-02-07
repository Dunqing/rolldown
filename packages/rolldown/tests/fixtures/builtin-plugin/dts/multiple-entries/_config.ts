import { defineTest } from 'rolldown-tests';
import { dtsPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  sequential: true,
  config: {
    input: ['a.ts', 'b.ts'],
    plugins: [dtsPlugin()],
  },
  async afterTest(output) {
    const chunks = output.output.filter((o) => o.type === 'chunk');

    // Should have JS chunks for both entries
    const jsChunks = chunks.filter((c) => c.fileName.endsWith('.js'));
    expect(jsChunks.length).toBe(2);

    // Should have DTS chunks (entry chunks + possibly shared chunks)
    const dtsChunks = chunks.filter((c) => c.fileName.endsWith('.d.ts'));
    expect(dtsChunks.length).toBeGreaterThanOrEqual(2);

    // Find each entry's DTS
    const aDts = dtsChunks.find((c) => c.fileName.startsWith('a'));
    const bDts = dtsChunks.find((c) => c.fileName.startsWith('b'));

    expect(aDts).toBeDefined();
    expect(bDts).toBeDefined();

    // Each should contain its specific interface
    expect(aDts!.code).toContain('ModuleA');
    expect(bDts!.code).toContain('ModuleB');

    // SharedType should be in some chunk (either inlined or in a separate shared chunk)
    const allDtsCode = dtsChunks.map((c) => c.code).join('\n');
    expect(allDtsCode).toContain('SharedType');
  },
});
