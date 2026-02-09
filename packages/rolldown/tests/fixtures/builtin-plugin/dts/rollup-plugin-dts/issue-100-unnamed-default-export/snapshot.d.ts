// index.d.d.ts
export default function <T extends object>(
  object: T,
  initializationObject: {
    [x in keyof T]: () => Promise<T[x]>;
  },
): Promise<void>;
