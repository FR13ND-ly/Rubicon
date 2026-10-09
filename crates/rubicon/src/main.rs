use axum::{Router, response::Html, routing::get};

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let app = Router::new().route("/", get(root));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;

    println!("Rubicon is running at http://127.0.0.1:3000");

    axum::serve(listener, app).await?;

    Ok(())
}

async fn root() -> Html<&'static str> {
    Html("<h1>Rubicon</h1>")
}
