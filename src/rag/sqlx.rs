use dotenv::dotenv;
use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use std::env;

use crate::rag::dataframes::SignalMessageWithVector;

use super::dataframes::SignalMessageWithEmbedding;

pub async fn setup_database() -> Result<Pool<Postgres>, sqlx::Error> {
    dotenv().ok();

    // Replace with your actual connection string
    let connection_string = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    // Create a connection pool
    let pool = PgPoolOptions::new()
        .max_connections(100)
        .connect(connection_string.as_str())
        .await?;

    // Install pgvector extension
    sqlx::query("CREATE EXTENSION IF NOT EXISTS vector;")
        .execute(&pool)
        .await?;

    // Install pgvectorscale extension
    sqlx::query("CREATE EXTENSION IF NOT EXISTS vectorscale CASCADE;")
        .execute(&pool)
        .await?;

    // Create table to store embeddings and metadata
    match sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS embeddings (
            id bigserial primary key,
            body text,
            direction text,
            contact text,
            group_name text,
            attachments text,
            tokens integer,
            embedding VECTOR(768),
            created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        "#,
    )
    .execute(&pool)
    .await {
        Ok(x) => x,
        Err(err) => panic!("{:?}",err),
    };

    Ok(pool)
}

pub async fn insert_embeddings_into_db(
    pool: &Pool<Postgres>,
    msg_to_encode: Vec<SignalMessageWithEmbedding>,
) -> Result<(), sqlx::Error> {
    for msg in msg_to_encode {
        match sqlx::query(
            r#"
            INSERT INTO embeddings (body,direction,contact,group_name,attachments,tokens,embedding)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(&msg.body)
        .bind(&msg.direction)
        .bind(&msg.contact)
        .bind(&msg.group_name)
        .bind(&msg.attachments)
        .bind(&msg.tokens)
        .bind(&msg.embedding)
        .execute(pool)
        .await {
            Ok(_) => {},
            Err(err) => println!("{}",err),
        }
    }

    Ok(())
}

pub async fn get_all_embeddings_from_db(
    pool: &Pool<Postgres>,
) -> Result<Vec<SignalMessageWithVector>, sqlx::Error> {
    let response: Vec<SignalMessageWithVector> = sqlx::query_as("SELECT * FROM embeddings")
        .fetch_all(pool)
        .await?;

    Ok(response)
}
