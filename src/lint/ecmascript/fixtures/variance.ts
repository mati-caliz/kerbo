export interface Box<out T, in U> {
  get(): T;
  set(value: U): void;
}

export interface Shaped<
  /** @ts-ignore la varianza */
  out Shape extends object = object,
> {
  shape: Shape;
  extra: any;
}
