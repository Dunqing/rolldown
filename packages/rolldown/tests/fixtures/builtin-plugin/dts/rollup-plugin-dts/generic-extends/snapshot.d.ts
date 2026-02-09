// index.d.d.ts
export type AnimatedProps<T> = T;
export type AnimatedComponent<T extends ElementType> = ForwardRefExoticComponent<
  AnimatedProps<ComponentPropsWithRef<T>>
>;
