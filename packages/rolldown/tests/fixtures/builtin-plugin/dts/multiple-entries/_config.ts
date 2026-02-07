import { defineTest } from 'rolldown-tests';
import { dtsPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  sequential: true,
  // TODO: Multiple entries with shared dependencies has timing issues with emit_chunk
  // The virtual DTS chunks are processed before all dependencies are captured
  skip: true,
  config: {
    input: ['a.ts', 'b.ts'],
    plugins: [dtsPlugin()],
  },
  async afterTest(output) {
    const chunks = output.output.filter((o) => o.type === 'chunk');

    // Should have JS chunks for both entries
    const jsChunks = chunks.filter((c) => c.fileName.endsWith('.js'));
    expect(jsChunks.length).toBe(2);

    // Should have DTS chunks for both entries
    const dtsChunks = chunks.filter((c) => c.fileName.endsWith('.d.ts'));
    expect(dtsChunks.length).toBe(2);

    // Find each entry's DTS
    const aDts = dtsChunks.find((c) => c.fileName.includes('a'));
    const bDts = dtsChunks.find((c) => c.fileName.includes('b'));

    expect(aDts).toBeDefined();
    expect(bDts).toBeDefined();

    // Each should contain its specific interface
    expect(aDts!.code).toContain('ModuleA');
    expect(bDts!.code).toContain('ModuleB');

    // Both should have SharedType inlined (since it's bundled)
    expect(aDts!.code).toContain('SharedType');
    expect(bDts!.code).toContain('SharedType');
  },
});
