use reqwest;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let response = reqwest::get("https://www.google.com").await?;
    let status = response.status();
    let body = response.text().await?;
    println!("Status: {status}");
    println!("Body length: {} bytes", body.len());
    Ok(())
}
