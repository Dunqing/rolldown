import { SharedType } from './shared';

export interface ModuleA {
  name: string;
  shared: SharedType;
}

export function createA(): ModuleA {
  return { name: 'A', shared: { id: 1 } };
}
