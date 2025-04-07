use actix_web::web;
use crate::api::handlers::{
    list_folders, create_folder, delete_folder,
    list_emails, list_unread_emails, get_email, move_email,
};
use core::imap::client::ImapSessionTrait;

pub fn configure_routes<S>(cfg: &mut web::ServiceConfig)
where
    S: ImapSessionTrait + 'static + Send + Sync,
{
    cfg.service(
        web::scope("/api")
            .route("/folders", web::get().to(list_folders::<S>))
            .route("/folders/{name}", web::post().to(create_folder::<S>))
            .route("/folders/{name}", web::delete().to(delete_folder::<S>))
            .route("/folders/{name}/emails", web::get().to(list_emails::<S>))
            .route("/folders/{name}/unread", web::get().to(list_unread_emails::<S>))
            .route("/folders/{name}/emails/{id}", web::get().to(get_email::<S>))
            .route("/folders/{source}/emails/{id}/move/{target}", web::post().to(move_email::<S>))
    );

    // TODO: Add routes for / and /api-docs
}
