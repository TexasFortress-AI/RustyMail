use actix_web::{
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    web,
    Error,
};
use futures_util::future::{self, LocalBoxFuture, Ready};
use std::sync::Arc;
use std::time::Instant;
use crate::dashboard::services::DashboardState;

// Middleware factory
#[derive(Clone)]
pub struct Metrics;

impl<S, B> Transform<S, ServiceRequest> for Metrics
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = MetricsMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        future::ready(Ok(MetricsMiddleware { service: Arc::new(service) }))
    }
}

// Middleware service
pub struct MetricsMiddleware<S> {
    service: Arc<S>,
}

impl<S, B> Service<ServiceRequest> for MetricsMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    actix_web::dev::forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let start_time = Instant::now();
        let service = Arc::clone(&self.service);
        
        // Get DashboardState, then the MetricsService Arc from it
        let metrics_service_arc = req.app_data::<web::Data<DashboardState>>().map(|d| d.metrics_service.clone());
            
        Box::pin(async move {
            if let Some(metrics) = metrics_service_arc {
                // Record request start before calling the next service
                metrics.record_request_start().await;

                // Call the next service in the chain
                let res = service.call(req).await?;

                // Record response time after the request is handled
                let duration = start_time.elapsed();
                metrics.record_response_time(duration).await;
                Ok(res)
            } else {
                // Metrics service not found, just pass through
                // Log a warning/error here if this shouldn't happen
                eprintln!("WARN: MetricsService not found in app_data within middleware!");
                service.call(req).await
            }
        })
    }
}
