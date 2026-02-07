/// <reference types="node" />
/// <reference path="./globals.d.ts" />

export interface ServerConfig {
  port: number;
  host: string;
}

export declare function createServer(config: ServerConfig): void;
