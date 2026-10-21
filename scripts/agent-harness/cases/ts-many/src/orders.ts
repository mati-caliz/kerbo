import { readFileSync } from "node:fs";
import { join } from "node:path";

export interface Order {
  id: string;
  total: number;
  notes?: string;
}

var counter = 0;

export function nextId(prefix: string): string {
  counter += 1;
  let label = prefix + "-" + counter;
  return label;
}

export function parse(raw: any): Order {
  let order = raw as Order;
  return order;
}

export function notesOf(order: Order): string {
  return order.notes!.trim();
}

export function totals(orders: Order[]): number {
  let sum = 0;
  for (const order of orders) {
    sum += order.total;
  }
  const unused = orders.length;
  return sum;
}

export function hasKey(order: Order, key: string): boolean {
  return order.hasOwnProperty(key);
}

export function safeParse(text: string): Order | undefined {
  try {
    return JSON.parse(text);
  } catch (error) {}
  return undefined;
}
