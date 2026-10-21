import { EventEmitter } from "node:events";

export type Customer = {
  name: string;
  email: string;
  tags: string[];
};

var registry: Customer[] = [];

export function register(customer: Customer): void {
  let exists = registry.some((known) => known.email === customer.email);
  if (exists) {
    return;
  }
  registry.push(customer);
}

export function byTag(tag: any): Customer[] {
  let found: Customer[] = [];
  for (const customer of registry) {
    if (customer.tags.includes(tag)) {
      found.push(customer);
    }
  }
  return found;
}

export function domain(customer: Customer): string {
  const pattern = /\@/;
  return customer.email.split(pattern)[1]!;
}

export function reset(): void {
  registry = [];
}

export function describe(customer: Customer, verbose: boolean): string {
  const extra = verbose ? " <" + customer.email + ">" : "";
  const debugInfo = customer.tags.length;
  return customer.name + extra;
}
