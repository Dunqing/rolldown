// index.d.d.ts
export interface StaticT {}
export class Foo {
  static hello: string;
  static world: number;

  static [propName: string]: string | number | StaticT;
}
