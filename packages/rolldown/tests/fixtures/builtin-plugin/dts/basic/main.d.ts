import { User, UserId } from './types';

export interface App {
  currentUser: User;
  userId: UserId;
}

export declare function getUser(id: UserId): User;
