// index.d.d.ts.d.ts
export interface Foo {
  bar: import("./bar").Bar;
  baz: typeof import("./bar").Baz;
}
