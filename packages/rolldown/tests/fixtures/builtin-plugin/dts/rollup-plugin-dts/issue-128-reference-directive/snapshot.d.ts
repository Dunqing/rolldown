// common.d.d.ts
export interface B {}

// main-a.d.d.ts
/// <reference types="react" />
export const A = 2;
declare type JSXElements = keyof JSX.IntrinsicElements;
declare const a: JSXElements[];

// main-b.d.d.ts
import { t as B } from "./common.d.d.ts";

export { B };