// index.d.d.ts
export interface GetT {}
export interface SetT {}
export interface Thing {
  get size(): GetT;
  set size(value: GetT | SetT | boolean);
}
