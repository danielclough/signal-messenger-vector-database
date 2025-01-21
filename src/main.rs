use signal_vector_db::{entry_point, rag::sqlx::setup_database, types::Args};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let pg_pool = setup_database().await.unwrap();
    
    // Start receiving.
    let args = Args::default();
    _ = entry_point(args, &pg_pool).await.unwrap();

    Ok(())
}
