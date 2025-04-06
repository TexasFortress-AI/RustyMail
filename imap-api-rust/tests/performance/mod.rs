use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tokio::runtime::Runtime;
use imap_api_rust::tests::common::setup_test_app_data;
use actix_web::test;
use crate::common::setup_test_app;
use std::time::Instant;

fn bench_connection_pooling(c: &mut Criterion) {
    c.bench_function("connection_pooling", |b| {
        let rt = Runtime::new().unwrap();
        b.iter(|| {
            rt.block_on(async {
                let client_data = setup_test_app_data();
                let client = client_data.lock().unwrap();
                black_box(client.list_folders().await);
            });
        });
    });
}

fn bench_concurrent_requests(c: &mut Criterion) {
    c.bench_function("concurrent_requests", |b| {
        let client_data = setup_test_app_data();
        let rt = Runtime::new().unwrap();
        b.iter(|| {
            rt.block_on(async {
                let client = client_data.lock().unwrap();
                let tasks = (0..10).map(|_| {
                    client.list_folders()
                });
                let _ = futures::future::join_all(tasks).await;
            });
        });
    });
}

fn bench_large_email_handling(c: &mut Criterion) {
    c.bench_function("large_email_handling", |b| {
        let client_data = setup_test_app_data();
        let rt = Runtime::new().unwrap();
        b.iter(|| {
            rt.block_on(async {
                let client = client_data.lock().unwrap();
                black_box(client.fetch("1").await);
            });
        });
    });
}

fn bench_batch_operations(c: &mut Criterion) {
    c.bench_function("batch_operations", |b| {
        let client_data = setup_test_app_data();
        let rt = Runtime::new().unwrap();
        b.iter(|| {
            rt.block_on(async {
                let client = client_data.lock().unwrap();
                black_box(client.search("ALL").await);
            });
        });
    });
}

criterion_group!(
    benches,
    bench_connection_pooling,
    bench_concurrent_requests,
    bench_large_email_handling,
    bench_batch_operations
);
criterion_main!(benches);

#[cfg(test)]
mod tests {
    use super::*;

    #[actix_web::test]
    async fn test_folder_list_performance() {
        let app = test::init_service(setup_test_app()).await;
        let start = Instant::now();
        
        let req = test::TestRequest::get().uri("/folders").to_request();
        let resp = test::call_service(&app, req).await;
        
        let duration = start.elapsed();
        assert!(resp.status().is_success());
        assert!(duration.as_secs_f64() < 1.0, "Folder list took too long: {:?}", duration);
    }

    #[actix_web::test]
    async fn test_email_list_performance() {
        let app = test::init_service(setup_test_app()).await;
        let start = Instant::now();
        
        let req = test::TestRequest::get().uri("/emails/INBOX").to_request();
        let resp = test::call_service(&app, req).await;
        
        let duration = start.elapsed();
        assert!(resp.status().is_success());
        assert!(duration.as_secs_f64() < 2.0, "Email list took too long: {:?}", duration);
    }
} 