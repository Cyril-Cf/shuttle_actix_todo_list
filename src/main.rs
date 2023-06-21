use actix_web::middleware::Logger;
use actix_web::HttpResponse;
use actix_web::{
    delete, error, get, post, put,
    web::{self, Json, ServiceConfig},
    Result,
};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use shuttle_actix_web::ShuttleActixWeb;
use shuttle_runtime::CustomError;
use sqlx::{Executor, FromRow, PgPool};

#[get("")]
async fn retrieve_all(state: web::Data<AppState>) -> Result<Json<Vec<Todo>>> {
    let todos = sqlx::query_as("SELECT * FROM todos")
        .fetch_all(&state.pool)
        .await
        .map_err(|e| error::ErrorBadRequest(e.to_string()))?;

    Ok(Json(todos))
}

#[derive(Deserialize)]
struct TodoUpdate {
    pub note: String,
    pub is_started: bool,
    pub is_finished: bool,
    pub owner_email: String,
    pub owner_password: String,
}

#[put("/{id}")]
async fn update(
    path: web::Path<i32>,
    todo: web::Json<TodoUpdate>,
    state: web::Data<AppState>,
) -> Result<Json<Todo>> {
    let todo_data = sqlx::query_as::<_, (String, String)>(
        "SELECT owner_email, owner_password FROM todos WHERE id = $1",
    )
    .bind(*path)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| error::ErrorBadRequest(e.to_string()))?;
    let (stored_email, stored_password) = match todo_data {
        Some(data) => data,
        None => return Err(error::ErrorNotFound("Todo not found")),
    };

    if stored_email != todo.owner_email || stored_password != todo.owner_password {
        return Err(error::ErrorForbidden("Invalid owner credentials"));
    }

    let updated_todo = sqlx::query_as("UPDATE todos SET note = $1, is_started = $2, is_finished = $3 WHERE id = $4 RETURNING id, note, date_time_created, date_time_to_complete_task, owner_email, owner_password, is_started, is_finished")
        .bind(&todo.note)
        .bind(todo.is_started)
        .bind(todo.is_finished)
        .bind(*path)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| error::ErrorBadRequest(e.to_string()))?;

    Ok(Json(updated_todo))
}

#[get("/{id}")]
async fn retrieve(path: web::Path<i32>, state: web::Data<AppState>) -> Result<Json<Todo>> {
    let todo = sqlx::query_as("SELECT * FROM todos WHERE id = $1")
        .bind(*path)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| error::ErrorBadRequest(e.to_string()))?;

    Ok(Json(todo))
}

#[derive(Deserialize)]
struct TodoDelete {
    pub owner_email: String,
    pub owner_password: String,
}

#[delete("/{id}")]
async fn delete_todo(
    todo: web::Json<TodoDelete>,
    path: web::Path<i32>,
    state: web::Data<AppState>,
) -> Result<HttpResponse> {
    let todo_data = sqlx::query_as::<_, (String, String)>(
        "SELECT owner_email, owner_password FROM todos WHERE id = $1",
    )
    .bind(*path)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| error::ErrorBadRequest(e.to_string()))?;
    let (stored_email, stored_password) = match todo_data {
        Some(data) => data,
        None => return Err(error::ErrorNotFound("Todo not found")),
    };

    if stored_email != todo.owner_email || stored_password != todo.owner_password {
        return Err(error::ErrorForbidden("Invalid owner credentials"));
    }

    sqlx::query("DELETE FROM todos WHERE id = $1")
        .bind(*path)
        .execute(&state.pool)
        .await
        .map_err(|e| error::ErrorInternalServerError(e.to_string()))?;

    Ok(HttpResponse::Ok().finish())
}

#[post("/reset-all")]
async fn reset_all(state: web::Data<AppState>) -> Result<HttpResponse> {
    state
        .pool
        .execute(include_str!("../schema.sql"))
        .await
        .map_err(|e| {
            eprintln!("Failed to reset database: {}", e);
            error::ErrorInternalServerError("Failed to reset database")
        })?;

    Ok(HttpResponse::Ok().finish())
}

#[post("")]
async fn add(todo: web::Json<TodoNew>, state: web::Data<AppState>) -> Result<Json<Todo>> {
    let date_time_to_complete_task =
        match NaiveDateTime::from_timestamp_opt(todo.date_time_to_complete_task_timestamp, 0) {
            Some(date_time) => date_time,
            None => return Err(error::ErrorBadRequest("Invalid timestamp".to_string())),
        };

    let todo = sqlx::query_as("INSERT INTO todos (note, date_time_created, date_time_to_complete_task, owner_email, owner_password, is_started, is_finished) VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id, note, date_time_created, date_time_to_complete_task, owner_email, owner_password, is_started, is_finished")
        .bind(&todo.note)
        .bind(chrono::Utc::now().naive_utc())
        .bind(date_time_to_complete_task)
        .bind(&todo.owner_email)
        .bind(&todo.owner_password)
        .bind(false)
        .bind(false)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| error::ErrorBadRequest(e.to_string()))?;

    Ok(Json(todo))
}

#[derive(Clone)]
struct AppState {
    pool: PgPool,
}

#[shuttle_runtime::main]
async fn actix_web(
    #[shuttle_shared_db::Postgres] pool: PgPool,
) -> ShuttleActixWeb<impl FnOnce(&mut ServiceConfig) + Send + Clone + 'static> {
    pool.execute(include_str!("../schema.sql"))
        .await
        .map_err(CustomError::new)?;

    let state = web::Data::new(AppState { pool });

    let config = move |cfg: &mut ServiceConfig| {
        cfg.service(
            web::scope("/todos")
                .wrap(Logger::default())
                .service(retrieve)
                .service(add)
                .service(retrieve_all)
                .service(update)
                .service(reset_all)
                .service(delete_todo)
                .app_data(state),
        );
    };

    Ok(config.into())
}

#[derive(Deserialize)]
struct TodoNew {
    pub note: String,
    pub date_time_to_complete_task_timestamp: i64,
    pub owner_email: String,
    pub owner_password: String,
}

#[derive(Serialize, Deserialize, FromRow)]
struct Todo {
    pub id: i32,
    pub note: String,
    pub date_time_created: NaiveDateTime,
    pub date_time_to_complete_task: NaiveDateTime,
    pub owner_email: String,
    pub owner_password: String,
    pub is_started: bool,
    pub is_finished: bool,
}
