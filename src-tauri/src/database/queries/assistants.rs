use uuid::Uuid;

use crate::database::{
    get_database_pool,
    models::{Assistant, AssistantDb, AssistantListResponse, CreateAssistantRequest, UpdateAssistantRequest},
};

/// Create a new assistant
pub async fn create_assistant(
    request: CreateAssistantRequest,
    created_by: Option<Uuid>,
) -> Result<Assistant, sqlx::Error> {
    let pool = get_database_pool()?;
    let pool = pool.as_ref();
    let assistant_id = Uuid::new_v4();

    let assistant_row: AssistantDb = sqlx::query_as(
        "INSERT INTO assistants (id, name, description, instructions, parameters, created_by, is_template) 
         VALUES ($1, $2, $3, $4, $5, $6, $7) 
         RETURNING id, name, description, instructions, parameters, created_by, is_template, is_active, created_at, updated_at"
    )
    .bind(assistant_id)
    .bind(&request.name)
    .bind(&request.description)
    .bind(&request.instructions)
    .bind(request.parameters.unwrap_or(serde_json::json!({
        "stream": true,
        "temperature": 0.7,
        "frequency_penalty": 0.7,
        "presence_penalty": 0.7,
        "top_p": 0.95,
        "top_k": 2
    })))
    .bind(created_by)
    .bind(request.is_template.unwrap_or(false))
    .fetch_one(pool)
    .await?;

    Ok(Assistant {
        id: assistant_row.id,
        name: assistant_row.name,
        description: assistant_row.description,
        instructions: assistant_row.instructions,
        parameters: Some(assistant_row.parameters),
        created_by: assistant_row.created_by,
        is_template: assistant_row.is_template,
        is_active: assistant_row.is_active,
        created_at: assistant_row.created_at,
        updated_at: assistant_row.updated_at,
    })
}

/// Get assistant by ID
pub async fn get_assistant_by_id(
    assistant_id: Uuid,
    requesting_user_id: Option<Uuid>,
) -> Result<Option<Assistant>, sqlx::Error> {
    let pool = get_database_pool()?;
    let pool = pool.as_ref();

    let assistant_row: Option<AssistantDb> = sqlx::query_as(
        "SELECT id, name, description, instructions, parameters, created_by, is_template, is_active, created_at, updated_at 
         FROM assistants 
         WHERE id = $1 AND is_active = true AND (is_template = true OR created_by = $2)"
    )
    .bind(assistant_id)
    .bind(requesting_user_id)
    .fetch_optional(pool)
    .await?;

    match assistant_row {
        Some(assistant_db) => Ok(Some(Assistant {
            id: assistant_db.id,
            name: assistant_db.name,
            description: assistant_db.description,
            instructions: assistant_db.instructions,
            parameters: Some(assistant_db.parameters),
            created_by: assistant_db.created_by,
            is_template: assistant_db.is_template,
            is_active: assistant_db.is_active,
            created_at: assistant_db.created_at,
            updated_at: assistant_db.updated_at,
        })),
        None => Ok(None),
    }
}

/// List assistants with pagination
pub async fn list_assistants(
    page: i32,
    per_page: i32,
    requesting_user_id: Option<Uuid>,
    admin_view: bool,
) -> Result<AssistantListResponse, sqlx::Error> {
    let pool = get_database_pool()?;
    let pool = pool.as_ref();
    let offset = (page - 1) * per_page;

    let (query, count_query) = if admin_view {
        // Admin can see only template assistants (created by admin)
        (
            "SELECT id, name, description, instructions, parameters, created_by, is_template, is_active, created_at, updated_at 
             FROM assistants 
             WHERE is_template = true 
             ORDER BY created_at DESC 
             LIMIT $1 OFFSET $2",
            "SELECT COUNT(*) FROM assistants WHERE is_template = true"
        )
    } else {
        // Regular users can see active template assistants and their own assistants
        (
            "SELECT id, name, description, instructions, parameters, created_by, is_template, is_active, created_at, updated_at 
             FROM assistants 
             WHERE is_active = true AND ((is_template = true) OR created_by = $3)
             ORDER BY created_at DESC 
             LIMIT $1 OFFSET $2",
            "SELECT COUNT(*) FROM assistants WHERE is_active = true AND ((is_template = true) OR created_by = $1)"
        )
    };

    // Get total count
    let total_row: (i64,) = if admin_view {
        sqlx::query_as(count_query)
            .fetch_one(pool)
            .await?
    } else {
        sqlx::query_as(count_query)
            .bind(requesting_user_id)
            .fetch_one(pool)
            .await?
    };
    let total = total_row.0;

    // Get assistants
    let assistant_rows: Vec<AssistantDb> = if admin_view {
        sqlx::query_as(query)
            .bind(per_page)
            .bind(offset)
            .fetch_all(pool)
            .await?
    } else {
        sqlx::query_as(query)
            .bind(per_page)
            .bind(offset)
            .bind(requesting_user_id)
            .fetch_all(pool)
            .await?
    };

    let assistants = assistant_rows
        .into_iter()
        .map(|assistant_db| Assistant {
            id: assistant_db.id,
            name: assistant_db.name,
            description: assistant_db.description,
            instructions: assistant_db.instructions,
            parameters: Some(assistant_db.parameters),
            created_by: assistant_db.created_by,
            is_template: assistant_db.is_template,
            is_active: assistant_db.is_active,
            created_at: assistant_db.created_at,
            updated_at: assistant_db.updated_at,
        })
        .collect();

    Ok(AssistantListResponse {
        assistants,
        total,
        page,
        per_page,
    })
}

/// Update assistant
pub async fn update_assistant(
    assistant_id: Uuid,
    request: UpdateAssistantRequest,
    requesting_user_id: Option<Uuid>,
    is_admin: bool,
) -> Result<Option<Assistant>, sqlx::Error> {
    let pool = get_database_pool()?;
    let pool = pool.as_ref();

    let where_clause = if is_admin {
        "WHERE id = $1"
    } else {
        "WHERE id = $1 AND created_by = $8"
    };

    let query = format!(
        "UPDATE assistants 
         SET name = COALESCE($2, name),
             description = COALESCE($3, description),
             instructions = COALESCE($4, instructions),
             parameters = COALESCE($5, parameters),
             is_template = COALESCE($6, is_template),
             is_active = COALESCE($7, is_active),
             updated_at = CURRENT_TIMESTAMP
         {} 
         RETURNING id, name, description, instructions, parameters, created_by, is_template, is_active, created_at, updated_at",
        where_clause
    );

    let assistant_row: Option<AssistantDb> = if is_admin {
        sqlx::query_as(&query)
            .bind(assistant_id)
            .bind(&request.name)
            .bind(&request.description)
            .bind(&request.instructions)
            .bind(&request.parameters)
            .bind(request.is_template)
            .bind(request.is_active)
            .fetch_optional(pool)
            .await?
    } else {
        sqlx::query_as(&query)
            .bind(assistant_id)
            .bind(&request.name)
            .bind(&request.description)
            .bind(&request.instructions)
            .bind(&request.parameters)
            .bind(request.is_template)
            .bind(request.is_active)
            .bind(requesting_user_id)
            .fetch_optional(pool)
            .await?
    };

    match assistant_row {
        Some(assistant_db) => Ok(Some(Assistant {
            id: assistant_db.id,
            name: assistant_db.name,
            description: assistant_db.description,
            instructions: assistant_db.instructions,
            parameters: Some(assistant_db.parameters),
            created_by: assistant_db.created_by,
            is_template: assistant_db.is_template,
            is_active: assistant_db.is_active,
            created_at: assistant_db.created_at,
            updated_at: assistant_db.updated_at,
        })),
        None => Ok(None),
    }
}

/// Delete assistant
pub async fn delete_assistant(
    assistant_id: Uuid,
    requesting_user_id: Option<Uuid>,
    is_admin: bool,
) -> Result<bool, sqlx::Error> {
    let pool = get_database_pool()?;
    let pool = pool.as_ref();

    let result = if is_admin {
        sqlx::query("DELETE FROM assistants WHERE id = $1")
            .bind(assistant_id)
            .execute(pool)
            .await?
    } else {
        sqlx::query("DELETE FROM assistants WHERE id = $1 AND created_by = $2")
            .bind(assistant_id)
            .bind(requesting_user_id)
            .execute(pool)
            .await?
    };

    Ok(result.rows_affected() > 0)
}

/// Get default assistant
pub async fn get_default_assistant() -> Result<Option<Assistant>, sqlx::Error> {
    let pool = get_database_pool()?;
    let pool = pool.as_ref();

    let assistant_row: Option<AssistantDb> = sqlx::query_as(
        "SELECT id, name, description, instructions, parameters, created_by, is_template, is_active, created_at, updated_at 
         FROM assistants 
         WHERE name = 'Default Assistant' AND is_template = true AND is_active = true 
         LIMIT 1"
    )
    .fetch_optional(pool)
    .await?;

    match assistant_row {
        Some(assistant_db) => Ok(Some(Assistant {
            id: assistant_db.id,
            name: assistant_db.name,
            description: assistant_db.description,
            instructions: assistant_db.instructions,
            parameters: Some(assistant_db.parameters),
            created_by: assistant_db.created_by,
            is_template: assistant_db.is_template,
            is_active: assistant_db.is_active,
            created_at: assistant_db.created_at,
            updated_at: assistant_db.updated_at,
        })),
        None => Ok(None),
    }
}