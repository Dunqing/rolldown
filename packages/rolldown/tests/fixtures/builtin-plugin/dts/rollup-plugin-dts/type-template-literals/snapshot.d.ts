// index.d.d.ts.d.ts
export type Color = "red" | "blue";
export type Quantity = "one" | "two";
export type VerticalAlignment = "top" | "middle" | "bottom";
export type HorizontalAlignment = "left" | "center" | "right";
export type SeussFish = `${Quantity | Color} fish`;
export declare function setAlignment(value: `${VerticalAlignment}-${HorizontalAlignment}`): void;
