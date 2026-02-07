import { SharedType } from './shared';

export interface ModuleB {
  value: number;
  shared: SharedType;
}

export function createB(): ModuleB {
  return { value: 42, shared: { id: 2 } };
}
