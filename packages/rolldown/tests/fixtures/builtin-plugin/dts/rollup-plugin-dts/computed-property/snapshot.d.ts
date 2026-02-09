// index.d.d.ts.d.ts
interface A {}
interface B {}
interface C {}
interface D {}
export type Klass = {
  [Aprop]?: A[];
  ["B"]: B;
  [0]: C;
  [Dprop]: D;
};
