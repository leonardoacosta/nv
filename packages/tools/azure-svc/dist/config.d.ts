export interface ServiceConfig {
    serviceName: string;
    servicePort: number;
    logLevel: string;
    corsOrigin: string;
    cloudpcHost: string;
}
export declare function loadConfig(): ServiceConfig;
