use chrono::{DateTime, Utc, Duration};
use std::collections::HashMap;
use tracing::{debug, warn};

use super::resource_scheduler::SLAStatus;

pub struct SLAManager {
    sla_policies: HashMap<String, SLAPolicy>,
    violation_history: HashMap<String, Vec<SLAViolation>>,
}

#[derive(Debug, Clone)]
pub struct SLAPolicy {
    pub resource_id: String,
    pub max_cpu_utilization: f64,
    pub max_memory_utilization: f64,
    pub max_response_time_ms: u64,
    pub min_availability_percent: f64,
    pub priority: SLAPriority,
    pub deadline_minutes: u32,
}

#[derive(Debug, Clone)]
pub enum SLAPriority {
    Critical,
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone)]
pub struct SLAViolation {
    pub resource_id: String,
    pub violation_type: ViolationType,
    pub severity: f64,
    pub timestamp: DateTime<Utc>,
    pub resolved: bool,
}

#[derive(Debug, Clone)]
pub enum ViolationType {
    CpuUtilization,
    MemoryUtilization,
    ResponseTime,
    Availability,
}

impl SLAManager {
    pub fn new() -> Self {
        Self {
            sla_policies: HashMap::new(),
            violation_history: HashMap::new(),
        }
    }
    
    pub async fn check_sla_compliance(&self, resource_id: &str) -> SLAStatus {
        debug!("Checking SLA compliance for resource {}", resource_id);
        
        if let Some(policy) = self.sla_policies.get(resource_id) {
            // Get current metrics for the resource
            let current_metrics = self.get_current_metrics(resource_id).await;
            
            let mut violations = Vec::new();
            let mut impact_score = 0.0;
            
            // Check CPU utilization
            if current_metrics.cpu_utilization > policy.max_cpu_utilization {
                violations.push(ViolationType::CpuUtilization);
                impact_score += self.calculate_impact_score(
                    current_metrics.cpu_utilization,
                    policy.max_cpu_utilization,
                    &policy.priority
                );
            }
            
            // Check memory utilization
            if current_metrics.memory_utilization > policy.max_memory_utilization {
                violations.push(ViolationType::MemoryUtilization);
                impact_score += self.calculate_impact_score(
                    current_metrics.memory_utilization,
                    policy.max_memory_utilization,
                    &policy.priority
                );
            }
            
            // Check response time
            if current_metrics.response_time_ms > policy.max_response_time_ms {
                violations.push(ViolationType::ResponseTime);
                impact_score += 0.3; // Fixed impact for response time violations
            }
            
            // Determine if critical based on priority and violations
            let is_critical = matches!(policy.priority, SLAPriority::Critical) && !violations.is_empty();
            
            SLAStatus {
                is_critical,
                impact_score,
                deadline_minutes: policy.deadline_minutes,
            }
        } else {
            // No SLA policy defined - use default
            SLAStatus {
                is_critical: false,
                impact_score: 0.0,
                deadline_minutes: 60,
            }
        }
    }
    
    pub fn add_sla_policy(&mut self, policy: SLAPolicy) {
        self.sla_policies.insert(policy.resource_id.clone(), policy);
    }
    
    pub fn record_violation(&mut self, violation: SLAViolation) {
        warn!("SLA violation recorded: {:?}", violation);
        
        self.violation_history
            .entry(violation.resource_id.clone())
            .or_insert_with(Vec::new)
            .push(violation);
    }
    
    pub fn get_violation_history(&self, resource_id: &str) -> Vec<&SLAViolation> {
        self.violation_history
            .get(resource_id)
            .map(|violations| violations.iter().collect())
            .unwrap_or_default()
    }
    
    pub fn calculate_sla_compliance_rate(&self, resource_id: &str, period_hours: u32) -> f64 {
        let cutoff_time = Utc::now() - Duration::hours(period_hours as i64);
        
        if let Some(violations) = self.violation_history.get(resource_id) {
            let recent_violations = violations.iter()
                .filter(|v| v.timestamp > cutoff_time)
                .count();
            
            // Simple compliance calculation
            let total_periods = period_hours * 60; // minutes
            let violation_periods = recent_violations * 5; // assume 5 min per violation
            
            ((total_periods as i64 - violation_periods as i64) as f64 / total_periods as f64 * 100.0)
                .max(0.0)
                .min(100.0)
        } else {
            100.0 // No violations = 100% compliance
        }
    }
    
    async fn get_current_metrics(&self, _resource_id: &str) -> ResourceMetrics {
        // Mock implementation - would get actual metrics from monitoring system
        ResourceMetrics {
            cpu_utilization: 45.0,
            memory_utilization: 60.0,
            response_time_ms: 150,
            availability_percent: 99.5,
        }
    }
    
    fn calculate_impact_score(&self, current: f64, threshold: f64, priority: &SLAPriority) -> f64 {
        let violation_ratio = (current - threshold) / threshold;
        let priority_multiplier = match priority {
            SLAPriority::Critical => 1.0,
            SLAPriority::High => 0.8,
            SLAPriority::Medium => 0.6,
            SLAPriority::Low => 0.4,
        };
        
        violation_ratio * priority_multiplier
    }
}

#[derive(Debug)]
struct ResourceMetrics {
    cpu_utilization: f64,
    memory_utilization: f64,
    response_time_ms: u64,
    availability_percent: f64,
}

impl Default for SLAManager {
    fn default() -> Self {
        Self::new()
    }
}
