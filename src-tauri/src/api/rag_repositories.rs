use axum::{
    extract::{Path, Query},
    http::StatusCode,
    Extension, Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::api::errors::{ApiResult, AppError};
use crate::api::middleware::AuthenticatedUser;
use crate::database::{
    models::{
        CreateRAGRepositoryRequest, RAGRepository, RAGRepositoryListResponse, UpdateRAGRepositoryRequest,
        RAGRepositoryConnectionTestResponse, DownloadRAGDatabaseFromRepositoryRequest,
        RAGDatabase,
    },
    queries::{rag_repositories, rag_providers},
};

#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    page: Option<i32>,
    per_page: Option<i32>,
}

// RAG Repository endpoints
pub async fn list_rag_repositories(
    Extension(_user): Extension<AuthenticatedUser>,
    Query(pagination): Query<PaginationQuery>,
) -> ApiResult<Json<RAGRepositoryListResponse>> {
    let response = rag_repositories::list_rag_repositories(pagination.page, pagination.per_page).await?;
    Ok(Json(response))
}

pub async fn get_rag_repository(
    Extension(_user): Extension<AuthenticatedUser>,
    Path(repository_id): Path<Uuid>,
) -> ApiResult<Json<RAGRepository>> {
    let repository = rag_repositories::get_rag_repository_by_id(repository_id)
        .await?
        .ok_or(AppError::not_found("RAG repository"))?;
    
    Ok(Json(repository))
}

pub async fn create_rag_repository(
    Extension(_user): Extension<AuthenticatedUser>,
    Json(request): Json<CreateRAGRepositoryRequest>,
) -> ApiResult<Json<RAGRepository>> {
    let repository = rag_repositories::create_rag_repository(request).await?;
    Ok(Json(repository))
}

pub async fn update_rag_repository(
    Extension(_user): Extension<AuthenticatedUser>,
    Path(repository_id): Path<Uuid>,
    Json(request): Json<UpdateRAGRepositoryRequest>,
) -> ApiResult<Json<RAGRepository>> {
    let repository = rag_repositories::update_rag_repository(repository_id, request).await?;
    Ok(Json(repository))
}

pub async fn delete_rag_repository(
    Extension(_user): Extension<AuthenticatedUser>,
    Path(repository_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    rag_repositories::delete_rag_repository(repository_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn test_rag_repository_connection(
    Extension(_user): Extension<AuthenticatedUser>,
    Path(repository_id): Path<Uuid>,
) -> ApiResult<Json<RAGRepositoryConnectionTestResponse>> {
    let repository = rag_repositories::get_rag_repository_by_id(repository_id)
        .await?
        .ok_or(AppError::not_found("RAG repository"))?;

    // TODO: Implement actual connection test logic
    // For now, we'll return a mock response
    let response = RAGRepositoryConnectionTestResponse {
        success: true,
        message: "Connection test successful".to_string(),
        available_databases_count: Some(0),
    };

    Ok(Json(response))
}

pub async fn list_rag_repository_databases(
    Extension(_user): Extension<AuthenticatedUser>,
    Path(repository_id): Path<Uuid>,
) -> ApiResult<Json<Vec<RAGDatabase>>> {
    let _repository = rag_repositories::get_rag_repository_by_id(repository_id)
        .await?
        .ok_or(AppError::not_found("RAG repository"))?;

    // TODO: Implement actual repository database listing
    // For now, return an empty list as this requires external API calls
    let databases = vec![];

    Ok(Json(databases))
}

pub async fn download_rag_database_from_repository(
    Extension(_user): Extension<AuthenticatedUser>,
    Json(request): Json<DownloadRAGDatabaseFromRepositoryRequest>,
) -> ApiResult<Json<RAGDatabase>> {
    // Validate that the target provider exists
    let _provider = rag_providers::get_rag_provider_by_id(request.target_provider_id)
        .await?
        .ok_or(AppError::not_found("Target RAG provider"))?;

    // Validate that the repository exists
    let _repository = rag_repositories::get_rag_repository_by_id(request.repository_id)
        .await?
        .ok_or(AppError::not_found("RAG repository"))?;

    // TODO: Implement actual download logic
    // For now, create a placeholder database
    let database_name = request.database_name.unwrap_or_else(|| "Downloaded Database".to_string());
    let database_alias = request.database_alias.unwrap_or_else(|| "downloaded-db".to_string());

    let create_request = crate::database::models::CreateRAGDatabaseRequest {
        name: database_name,
        alias: database_alias,
        description: Some("Downloaded from repository".to_string()),
        enabled: Some(true),
        collection_name: None,
        embedding_model: None,
        chunk_size: Some(1000),
        chunk_overlap: Some(200),
        capabilities: None,
        settings: None,
    };

    let database = rag_providers::create_rag_database(request.target_provider_id, create_request).await?;
    Ok(Json(database))
}