export interface User {
  id: number;
  name: string;
}

export function createUser(name: string): User {
  return { id: 1, name };
}
