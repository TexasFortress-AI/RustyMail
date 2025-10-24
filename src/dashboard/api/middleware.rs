// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use actix_web::{
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    web,
    Error,
    http::header,
};
use futures_util::future::{self, LocalBoxFuture, Ready};
use std::sync::Arc;
use std::time::Instant;
use crate::dashboard::services::DashboardState;
use crate::dashboard::api::models::ClientType;

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

        // Extract client information from request
        let user_agent = req.headers()
            .get(header::USER_AGENT)
            .and_then(|h| h.to_str().ok())
            .map(String::from);
        let ip_address = req.peer_addr().map(|addr| addr.ip().to_string());
        let path = req.path().to_string();

        // Get DashboardState for both metrics and client management
        let dashboard_state = req.app_data::<web::Data<DashboardState>>().cloned();

        Box::pin(async move {
            if let Some(state) = dashboard_state {
                // Record request start for metrics
                state.metrics_service.record_request_start().await;

                // Track API client if this is an API request (not SSE)
                let client_id = if path.starts_with("/api/") && !path.contains("/events") {
                    let client_id = state.client_manager.register_client(
                        ClientType::Api,
                        ip_address,
                        user_agent,
                    ).await;
                    Some(client_id)
                } else {
                    None
                };

                // Call the next service in the chain
                let res = service.call(req).await;

                // Record response time after the request is handled
                let duration = start_time.elapsed();
                state.metrics_service.record_response_time(duration).await;

                // Update client activity if this was an API request
                if let Some(id) = client_id {
                    state.client_manager.update_client_activity(&id).await;
                }

                res
            } else {
                // Dashboard state not found, just pass through
                eprintln!("WARN: DashboardState not found in app_data within middleware!");
                service.call(req).await
            }
        })
    }
}
