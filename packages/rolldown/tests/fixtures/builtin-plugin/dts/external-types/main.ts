// Test external type imports
import type { RolldownOutput } from 'rolldown';

export interface MyOutput {
  result: RolldownOutput;
  name: string;
}

export function processOutput(output: RolldownOutput): MyOutput {
  return { result: output, name: 'test' };
}
