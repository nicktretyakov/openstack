use thiserror::Error;

#[derive(Error, Debug)]
pub enum OpenStackError {
    #[error("Authentication failed: {0}")]
    AuthError(String),
    
    #[error("API request failed with status {status}: {message}")]
    ApiError {
        status: u16,
        message: String,
    },
    
    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),
    
    #[error("Invalid configuration: {0}")]
    ConfigError(String),
}

#[derive(Error, Debug)]
pub enum MetricsError {
    #[error("Collection failed: {0}")]
    CollectionError(String),
    
    #[error("Kafka producer error: {0}")]
    KafkaError(String),
    
    #[error("Processing error: {0}")]
    ProcessingError(String),
}

#[derive(Error, Debug)]
pub enum MLError {
    #[error("Model loading failed: {0}")]
    ModelLoadError(String),
    
    #[error("Inference failed: {0}")]
    InferenceError(String),
    
    #[error("Training failed: {0}")]
    TrainingError(String),
}

#[derive(Error, Debug)]
pub enum SchedulerError {
    #[error("Scheduling decision failed: {0}")]
    DecisionError(String),
    
    #[error("Resource placement failed: {0}")]
    PlacementError(String),
    
    #[error("SLA violation: {0}")]
    SLAViolation(String),
}
