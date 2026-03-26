export interface ServiceHealthReport {
  name: string;
  url: string;
  status: "healthy" | "unhealthy" | "unreachable";
  uptime_secs?: number;
  latency_ms: number;
  error?: string;
}

export interface FleetHealthSummary {
  total: number;
  healthy: number;
  unhealthy: number;
  unreachable: number;
}

export interface SelfAssessmentResult {
  generated_at: string;
  memory_topic_count: number;
  recent_message_count: number;
  fleet_health: FleetHealthSummary;
  observations: string[];
  suggestions: string[];
}
