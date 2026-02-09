// common.d.d.ts.d.ts
export interface A {}
export interface B {}

// main-a.d.d.ts
import { t as A } from "./common.d.d.ts";

export { A };
// main-b.d.d.ts
import { n as B } from "./common.d.d.ts";

export { B };